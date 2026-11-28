//! Dumps the pod exec pane to a self-contained HTML file for visual review.
//!
//! Usage: `cargo run -p openkite-ui --example dump_terminal -- <out.html> [connecting|open|exited|dropped]`

use dioxus::prelude::*;
use openkite_api::exec::{ExecEvent, ExecExitCode, ShellChannel};
use openkite_ui::terminal::TerminalSession;
use openkite_ui::terminal_view::{Terminal, TerminalAction};
use openkite_ui::{AppShell, Connection, Counts};

fn session_for(state: &str) -> TerminalSession {
    let mut session =
        TerminalSession::new("prod", "api-7d9c4f8b6-2xqzt", 100, 30).with_container("main");
    match state {
        "connecting" => {}
        "exited" => {
            session.apply_event(ExecEvent::Output {
                channel: ShellChannel::Stdout,
                sequence: 0,
                bytes: b"/ # ls\nbin  etc  home  tmp\n/ # exit\n".to_vec(),
            });
            session.apply_event(ExecEvent::Exit(ExecExitCode::Code(0)));
        }
        "dropped" => {
            session.apply_event(ExecEvent::Output {
                channel: ShellChannel::Stdout,
                sequence: 0,
                bytes: b"/ # tail -f /var/log/app.log\n".to_vec(),
            });
            session.apply_event(ExecEvent::Dropped {
                channel: ShellChannel::Stdout,
                count: 128,
            });
        }
        _ => {
            session.apply_event(ExecEvent::Output {
                channel: ShellChannel::Stdout,
                sequence: 0,
                bytes: b"/ # ls\nbin  etc  home  tmp  usr  var\n/ # ".to_vec(),
            });
        }
    }
    session
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    session: TerminalSession,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_action = use_callback(|_: TerminalAction| {});
    rsx! {
        AppShell {
            counts: Counts::new().with("pods", 42),
            active_path: "/workloads/pods/api-7d9c4f8b6-2xqzt/shell".to_string(),
            cluster: "prod-east-1".to_string(),
            connection: Connection::Live,
            on_settings: None,
            children: rsx! {
                div { class: "page-head", h1 { "Shell" } }
                Terminal { session: props.session.clone(), on_action }
            },
        }
    }
}

fn main() {
    let state = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "open".to_string());
    let mut dom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            session: session_for(&state),
        },
    );
    dom.rebuild_in_place();
    let mut body = dioxus_ssr::render(&dom);

    let css = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/console.css"))
        .unwrap()
        .replace(
            "url(\"/assets/fonts/",
            &format!("url(\"file://{}/assets/fonts/", env!("CARGO_MANIFEST_DIR")),
        );
    let sprite =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons.svg")).unwrap();
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    body = body.replace(&format!("{crate_dir}/assets/icons.svg#"), "#");
    body = body.replace(&format!("{crate_dir}#"), "#");

    let html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>OpenKite shell</title><style>{css}</style></head>\n<body>{sprite}\n{body}</body></html>\n"
    );
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "terminal.html".to_string());
    std::fs::write(&out, html).unwrap();
    println!("wrote {out}");
}
