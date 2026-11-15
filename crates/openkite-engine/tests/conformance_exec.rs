//! Conformance diff of the pod exec driver against the `kubectl` CLI.
//!
//! Three acceptance rows, each driven twice on the same pod: once through
//! [`PodExecSession`] and once through `kubectl exec`, so a regression in our driver shows up
//! as a *difference* rather than as a self-consistent assertion:
//!
//! - **exit code** — `sh -c 'exit 7'`; our surfaced [`ExecExit`] must equal the process exit
//!   status `kubectl` reports for the identical command.
//! - **stdin EOF** — a `cat` gated on end-of-input. Both sides write a probe, close stdin and
//!   must see the command finish with the same output and the same zero exit. The driver's
//!   session must survive the close (a later write reports `StdinClosed`, not a dead session).
//! - **resize** — a tty session. The remote `stty size` must echo the rows/columns we sent,
//!   twice, with a real change in between so the assertion cannot pass on a pty default.
//!   `kubectl` has no resize interface (it copies the size of its *own* local tty), so this
//!   row is verified end-to-end against the container rather than diffed against the CLI.
//!
//! The test needs a live cluster and the `kubectl` binary, so it is ignored by default and
//! un-ignored by the conformance workflow, which brings up a kind cluster first.
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use kube::Client;
use openkite_engine::driver::error::DriverError;
use openkite_engine::driver::{ExecExit, PodExecRequest, PodExecSession};
use openkite_engine::exec::ExecState;
use tokio::time::sleep;

/// Namespace the fixture pod lives in; created and removed by the test.
const NAMESPACE: &str = "okt-exec-conformance";
/// Fixture pod name.
const POD: &str = "exec-conformance";
/// Image with a shell and `stty`, which the resize row needs.
const IMAGE: &str = "busybox:1.36";
/// Ceiling for each individual wait, so a wedged session fails the job instead of hanging it.
const ROW_DEADLINE: Duration = Duration::from_secs(30);
/// Ceiling for the whole test, including fixture creation and the image pull.
const TEST_DEADLINE: Duration = Duration::from_secs(600);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a live cluster and kubectl: creates a pod and diffs exec against the kubectl CLI"]
async fn exec_conformance_matches_kubectl() {
    let outcome = tokio::time::timeout(TEST_DEADLINE, run()).await;
    // The kind cluster is disposable, so a failed cleanup is not worth masking a real result.
    delete_fixture().await;
    outcome.expect("exec conformance exceeded its deadline");
}

/// Runs the three rows against one fixture pod and prints a comparison line per row.
async fn run() {
    require_kubectl().await;
    create_fixture().await;

    let client = Client::try_default()
        .await
        .expect("kube client from the environment");
    let exit_code = exit_code_row(client.clone()).await;
    let stdin_eof = stdin_eof_row(client.clone()).await;
    let resize = resize_row(client).await;

    println!("\nexec conformance: ours vs kubectl");
    println!(
        "  row=exit_code    ours={:?} kubectl={:?} verdict={}",
        exit_code.0,
        exit_code.1,
        verdict(exit_code.0 == exit_code.1)
    );
    println!(
        "  row=stdin_eof    ours={:?} kubectl={:?} verdict={}",
        stdin_eof.0,
        stdin_eof.1,
        verdict(stdin_eof.0 == stdin_eof.1)
    );
    println!(
        "  row=resize       ours={:?} kubectl=<no resize interface; verified end-to-end> verdict={}",
        resize,
        verdict(true)
    );
    println!();
}

fn verdict(matched: bool) -> &'static str {
    if matched {
        "match"
    } else {
        "MISMATCH"
    }
}

/// Refuses to run without `kubectl`: a missing binary would otherwise look like a cluster fault.
async fn require_kubectl() {
    let output = kubectl(&["version", "--client"]).await;
    assert!(
        output.status.success(),
        "kubectl is required for the conformance diff, but `kubectl version --client` failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

/// Creates the namespace and a sleeping pod, then waits for it to be Ready.
async fn create_fixture() {
    // A re-run against the same cluster may already own the namespace; that is not a failure.
    let namespace = kubectl(&["create", "namespace", NAMESPACE]).await;
    let stderr = String::from_utf8_lossy(&namespace.stderr).to_string();
    assert!(
        namespace.status.success() || stderr.contains("AlreadyExists"),
        "could not create namespace {NAMESPACE}: {stderr}"
    );

    kubectl(&["delete", "pod", POD, "-n", NAMESPACE, "--ignore-not-found"]).await;

    let run = kubectl(&[
        "run",
        POD,
        "-n",
        NAMESPACE,
        "--image",
        IMAGE,
        "--restart",
        "Never",
        "--command",
        "--",
        "sleep",
        "3600",
    ])
    .await;
    assert!(
        run.status.success(),
        "could not create fixture pod: {}",
        String::from_utf8_lossy(&run.stderr).trim()
    );

    let ready = kubectl(&[
        "wait",
        "--for=condition=Ready",
        &format!("pod/{POD}"),
        "-n",
        NAMESPACE,
        "--timeout=180s",
    ])
    .await;
    assert!(
        ready.status.success(),
        "fixture pod never became Ready: {}",
        String::from_utf8_lossy(&ready.stderr).trim()
    );
}

/// Best-effort removal of everything the test created.
async fn delete_fixture() {
    let output = kubectl(&["delete", "namespace", NAMESPACE, "--ignore-not-found"]).await;
    if !output.status.success() {
        eprintln!(
            "warning: could not delete namespace {NAMESPACE}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
}

/// Exit status row: the code our session surfaces must be the code `kubectl` reports.
async fn exit_code_row(client: Client) -> (Option<i32>, Option<i32>) {
    let command = ["sh", "-c", "exit 7"];

    let ours = execute(client, &command).await;
    let ours = match ours {
        Ok(exit) => exit_code(exit),
        Err(error) => panic!("driver exec failed for `sh -c 'exit 7'`: {error}"),
    };

    let theirs = kubectl(&["exec", POD, "-n", NAMESPACE, "--", "sh", "-c", "exit 7"]).await;
    let theirs = theirs.status.code();

    assert_eq!(
        ours, theirs,
        "exit code diverged from kubectl: ours={ours:?} kubectl={theirs:?}"
    );
    assert_eq!(ours, Some(7), "expected the command's own exit status 7");
    (ours, theirs)
}

/// Stdin EOF row: `cat` only finishes once the server is told stdin closed.
async fn stdin_eof_row(client: Client) -> (Option<i32>, Option<i32>) {
    let mut session = PodExecSession::open(
        client,
        PodExecRequest::new(NAMESPACE, POD, ["sh", "-c", "cat > /dev/null && echo eof"]),
    )
    .await
    .expect("open stdin-eof exec session");

    session
        .write_stdin(b"probe\n")
        .await
        .expect("write to stdin while it is open");
    session.close_stdin();
    assert!(
        !session.stdin_is_open(),
        "close_stdin left the session's stdin open"
    );

    // The session must outlive its stdin: the command is still running here, and only a live
    // session can report a refused write instead of a torn-down one.
    let refused = session.write_stdin(b"after-eof\n").await;
    assert!(
        matches!(refused, Err(DriverError::StdinClosed)),
        "writing after close_stdin should report StdinClosed, got {refused:?}"
    );

    let text = drain_stdout(&session, Some("eof"), ROW_DEADLINE).await;
    let exit = session.wait().await.expect("wait for stdin-eof session");
    let ours = exit_code(exit);
    assert_eq!(ours, Some(0), "`cat > /dev/null && echo eof` should exit 0");
    assert!(
        text.contains("eof"),
        "closing stdin did not let the remote command reach EOF; output was {text:?}"
    );

    // Same command through kubectl: write the same probe, then drop the piped stdin so the CLI
    // sees the same end-of-input and forwards it.
    let theirs = kubectl_with_stdin(
        &[
            "exec",
            "-i",
            POD,
            "-n",
            NAMESPACE,
            "--",
            "sh",
            "-c",
            "cat > /dev/null && echo eof",
        ],
        b"probe\n",
    )
    .await;
    let their_text = String::from_utf8_lossy(&theirs.stdout).to_string();
    let their_exit = theirs.status.code();
    assert!(
        their_text.contains("eof"),
        "kubectl baseline did not reach EOF either: {their_text:?}"
    );

    assert_eq!(
        ours, their_exit,
        "stdin-eof exit code diverged from kubectl: ours={ours:?} kubectl={their_exit:?}"
    );
    (ours, their_exit)
}

/// Resize row: a tty session whose remote `stty size` must report the sizes we send.
async fn resize_row(client: Client) -> String {
    let request = PodExecRequest::new(
        NAMESPACE,
        POD,
        [
            "sh",
            "-c",
            // Two reads gate the two reports; the remote sleeps after each read so the resize
            // frame settles on the pty before `stty` samples it.
            "read -r _; sleep 1; stty size; read -r _; sleep 1; stty size",
        ],
    )
    .tty(true);
    let mut session = PodExecSession::open(client, request)
        .await
        .expect("open tty exec session");

    let first = resized_size(&mut session, 133, 41).await;
    let second = resized_size(&mut session, 97, 33).await;

    assert_eq!(first, "41 133", "remote pty did not adopt the first size");
    assert_eq!(second, "33 97", "remote pty did not adopt the second size");

    let exit = session.wait().await.expect("wait for tty session");
    assert_eq!(
        exit_code(exit),
        Some(0),
        "the tty session's command should exit 0"
    );
    format!("{first} then {second}")
}

/// Sends one size, triggers the remote `stty size`, and returns its trimmed report.
async fn resized_size(session: &mut PodExecSession, width: u16, height: u16) -> String {
    session
        .resize(width, height)
        .await
        .expect("send terminal size on a tty session");
    // Let the resize forwarder push the frame before the command samples the pty.
    sleep(Duration::from_millis(300)).await;
    session
        .write_stdin(b"\n")
        .await
        .expect("release the remote read");

    // `stty size` reports "<rows> <columns>", so the expected report is height then width.
    let expected = format!("{height} {width}");
    let text = drain_stdout(session, Some(&expected), ROW_DEADLINE).await;
    assert!(
        text.contains(&expected),
        "remote pty never reported {expected:?} rows/columns; it said {text:?}"
    );

    text.lines()
        .map(str::trim)
        .filter(|line| {
            let mut parts = line.split_whitespace();
            matches!(
                (parts.next(), parts.next(), parts.next()),
                (Some(rows), Some(columns), None)
                    if rows.parse::<u16>().is_ok() && columns.parse::<u16>().is_ok()
            )
        })
        .next_back()
        .unwrap_or("")
        .to_owned()
}

/// Opens a session, drains it, and returns how the command ended.
async fn execute(client: Client, command: &[&str]) -> Result<ExecExit, DriverError> {
    let session = PodExecSession::open(
        client,
        PodExecRequest::new(NAMESPACE, POD, command.to_vec()),
    )
    .await?;
    drain_stdout(&session, None, ROW_DEADLINE).await;
    session.wait().await
}

fn exit_code(exit: ExecExit) -> Option<i32> {
    match exit {
        ExecExit::Code(code) => Some(code),
        ExecExit::Unknown => None,
    }
}

/// Drains stdout frames until `wanted` shows up, the pump closes, or the deadline passes.
async fn drain_stdout(
    session: &PodExecSession,
    wanted: Option<&str>,
    deadline: Duration,
) -> String {
    let started = Instant::now();
    let mut bytes = Vec::new();
    loop {
        while let Some(frame) = session.stdout().sink().pop().await {
            bytes.extend_from_slice(&frame.bytes);
        }

        let text = String::from_utf8_lossy(&bytes);
        let found = wanted.is_some_and(|needle| text.contains(needle));
        let closed = session.stdout().state() == ExecState::Closed;
        if found || (wanted.is_none() && closed) || started.elapsed() >= deadline {
            // The pump marks the stream closed after its final push, so a frame can land
            // between the drain above and this check; take it before handing the bytes back.
            let mut tail = Vec::new();
            while let Some(frame) = session.stdout().sink().pop().await {
                tail.extend_from_slice(&frame.bytes);
            }
            drop(text);
            bytes.extend_from_slice(&tail);
            return String::from_utf8_lossy(&bytes).into_owned();
        }
        sleep(Duration::from_millis(20)).await;
    }
}

/// Runs `kubectl` to completion and hands back its output, whatever its status.
async fn kubectl(args: &[&str]) -> Output {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    tokio::task::spawn_blocking(move || {
        Command::new("kubectl")
            .args(&owned)
            .output()
            .expect("spawn kubectl")
    })
    .await
    .expect("kubectl task panicked")
}

/// Runs `kubectl` with bytes written to a piped stdin that is closed before the wait.
async fn kubectl_with_stdin(args: &[&str], input: &[u8]) -> Output {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let input = input.to_vec();
    tokio::task::spawn_blocking(move || {
        let mut child = Command::new("kubectl")
            .args(&owned)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn kubectl with piped stdin");
        {
            let mut stdin = child.stdin.take().expect("piped stdin handle");
            stdin.write_all(&input).expect("write to kubectl stdin");
        } // Dropping the handle closes stdin, which is the EOF signal.
        child.wait_with_output().expect("wait for kubectl")
    })
    .await
    .expect("kubectl task panicked")
}
