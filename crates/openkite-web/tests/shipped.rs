//! The artifact a release image runs: the compiled host, booted the way the
//! image boots it, over a real socket.
//!
//! The route tests mount the router in-process, so they never exercise the two
//! settings the image passes in (`OPENKITE_ADDR`, `OPENKITE_WEB_ROOT`), and
//! they never cross the process boundary a browser console crosses. The
//! runtime stage copies this binary and a bundle, then starts it on `:8080`
//! with `OPENKITE_WEB_ROOT=/bundle`; that is what this boots.

mod support;

use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use support::{raw_http, raw_http_bytes};

/// The client glue wasm-bindgen writes for `openkite-web-client.wasm`, and the
/// wasm module it imports. Both land in `OPENKITE_WEB_ROOT` in the image.
const CLIENT_JS: &[u8] = b"export default function init() {}\n";
const CLIENT_WASM: &[u8] = b"\0asm\x01\0\0\0";

/// The client bundle URL a document serves, read off the page the way a browser
/// resolves it. The host versions the path by the hash of the staged bytes
/// (OKT-195), so the test cannot assume the value — only that it is versioned.
fn client_import(page: &str) -> String {
    const OPEN: &str = "import init from \"";
    let start = page
        .find(OPEN)
        .unwrap_or_else(|| panic!("the document imports no client bundle: {page}"))
        + OPEN.len();
    let rest = &page[start..];
    rest[..rest.find('"').expect("the import is a quoted URL")].to_string()
}

/// Every route the sidebar links to, plus a path below one of them: in a
/// browser each is a document navigation, so each has to answer the console.
const CONSOLE_ROUTES: [&str; 7] = [
    "/",
    "/cluster",
    "/workloads",
    "/config",
    "/logs",
    "/terminal",
    "/workloads/pods/probe-pod",
];

/// The host process, killed when the test drops it.
struct Host(Child);

impl Host {
    fn boot(web_root: &Path, kubeconfig: &Path, log: &Path) -> (Self, SocketAddr) {
        let addr = free_addr();
        let output = std::fs::File::create(log).expect("create the host log");
        let child = Command::new(env!("CARGO_BIN_EXE_openkite-web"))
            .env("OPENKITE_ADDR", addr.to_string())
            .env("OPENKITE_WEB_ROOT", web_root)
            .env("KUBECONFIG", kubeconfig)
            .stdout(Stdio::from(output.try_clone().expect("clone the log")))
            .stderr(Stdio::from(output))
            .spawn()
            .expect("spawn the host");
        (Self(child), addr)
    }

    /// Wait for the listener, or return the child's failure reason.
    async fn wait_until_serving(&mut self, addr: SocketAddr, log: &Path) -> Result<(), String> {
        for _ in 0..100 {
            if let Some(status) = self.0.try_wait().expect("poll the host") {
                return Err(format!(
                    "host exited before serving ({status}): {}",
                    tail(log)
                ));
            }
            if tokio::net::TcpStream::connect(addr).await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(format!("host did not answer on {addr}: {}", tail(log)))
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn the_shipped_binary_serves_the_console_on_every_deep_link() {
    let web_root = image_web_root();
    let scratch = tempfile::tempdir().expect("tempdir");
    let log = scratch.path().join("host.log");
    // A free-port probe cannot reserve the address across child startup.
    // Retry only bind collisions; fail immediately for unrelated exits.
    for attempt in 0..8 {
        let (mut host, addr) = Host::boot(web_root.path(), &kubeconfig(scratch.path()), &log);
        match host.wait_until_serving(addr, &log).await {
            Ok(()) => {
                for route in CONSOLE_ROUTES {
                    let response = raw_http(addr, "GET", route, None).await;
                    assert!(response.contains("200 OK"), "{route}: {response}");
                    assert!(
                        response.contains("data-surface=\"app\""),
                        "{route} did not render the console: {response}"
                    );
                    // Versioned by the hash of the staged bytes, so the page
                    // boots the wasm from the build that rendered it (OKT-195).
                    let import = client_import(&response);
                    assert!(
                        import.starts_with("/assets/bundle/")
                            && import.ends_with("/openkite-web-client.js"),
                        "{route} boots an unversioned client bundle ({import}): {response}"
                    );
                }
                return;
            }
            Err(error) if error.contains("Address already in use") && attempt < 7 => continue,
            Err(error) => panic!("{error}"),
        }
    }
    unreachable!("retry loop returns or panics");
}

#[tokio::test]
async fn the_shipped_binary_serves_the_bundle_its_console_boots() {
    let web_root = image_web_root();
    let scratch = tempfile::tempdir().expect("tempdir");
    let log = scratch.path().join("host.log");
    let (mut host, addr) = Host::boot(web_root.path(), &kubeconfig(scratch.path()), &log);
    host.wait_until_serving(addr, &log)
        .await
        .expect("host serves");

    // Ask the page which bundle it boots, then fetch exactly that: the URL a
    // browser would use, not one the test guessed.
    let document = raw_http(addr, "GET", "/", None).await;
    let client_url = client_import(&document);
    assert!(
        client_url.starts_with("/assets/bundle/"),
        "the document does not name a versioned bundle: {client_url}"
    );

    let client = raw_http_bytes(addr, "GET", &client_url, None).await;
    assert!(
        String::from_utf8_lossy(&client).contains("200 OK"),
        "the client glue is not served: {}",
        String::from_utf8_lossy(&client)
    );
    assert!(
        String::from_utf8_lossy(&client).contains("public, max-age=31536000, immutable"),
        "a hashed asset must be cacheable for good: {}",
        String::from_utf8_lossy(&client)
    );
    assert!(
        client.ends_with(CLIENT_JS),
        "the client glue did not round-trip byte for byte"
    );

    let wasm_url = client_url.replace("openkite-web-client.js", "openkite-web-client_bg.wasm");
    let wasm = raw_http_bytes(addr, "GET", &wasm_url, None).await;
    assert!(
        wasm.ends_with(CLIENT_WASM),
        "the wasm module did not round-trip byte for byte"
    );
    assert!(
        String::from_utf8_lossy(&wasm).contains("application/wasm"),
        "the module must be served as application/wasm: {}",
        String::from_utf8_lossy(&wasm)
    );

    // Bytes from another build are not under this one's name.
    let stale = "openkite-web-client.js";
    let stale = raw_http_bytes(
        addr,
        "GET",
        &format!("/assets/bundle/deadbeefdeadbeef/{stale}"),
        None,
    )
    .await;
    assert!(
        String::from_utf8_lossy(&stale).contains("404"),
        "an unknown bundle version must 404, not serve this build: {}",
        String::from_utf8_lossy(&stale)
    );
}

/// The web root the runtime image stages: the wasm-bindgen output for the
/// client, and no static document — the console document comes from the binary.
fn image_web_root() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(root.path().join("openkite-web-client.js"), CLIENT_JS).expect("write the glue");
    std::fs::write(root.path().join("openkite-web-client_bg.wasm"), CLIENT_WASM)
        .expect("write the wasm");
    root
}

/// A kubeconfig the host's client accepts, pointed at a closed port: the client
/// only has to build, nothing in these tests talks to a cluster.
fn kubeconfig(dir: &Path) -> PathBuf {
    let path = dir.join("kubeconfig");
    std::fs::write(
        &path,
        "apiVersion: v1\nkind: Config\n\
         clusters:\n  - name: host-test\n    cluster:\n      server: http://127.0.0.1:9\n\
         contexts:\n  - name: host-test\n    context:\n      cluster: host-test\n      user: host-test\n\
         users:\n  - name: host-test\n    user: {}\n\
         current-context: host-test\n",
    )
    .expect("write the kubeconfig");
    path
}

/// A port nothing holds: bound once to let the kernel pick, then released.
fn free_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let addr = listener.local_addr().expect("read the port");
    drop(listener);
    addr
}

fn tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(20)..].join("\n")
}
