//! Cluster shell pane: renders a [`crate::terminal::TerminalSession`] and reports keystrokes.
//!
//! Mirrors `logs_view`'s split from its state module: this file touches no socket and reads
//! only what the caller already applied to the session.

use dioxus::prelude::*;

use crate::terminal::{TerminalSession, TerminalStatus};

/// What the pane asks its caller to do with a line of input, a resize, or a detach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAction {
    /// The shell pressed enter on a line; the caller sends it as `ExecCommand::Stdin`.
    Submit(String),
    /// The viewport's cell size changed; the caller sends `ExecCommand::Resize`.
    Resize(u16, u16),
    /// The user closed the pane; the caller sends `ExecCommand::Detach`.
    Detach,
}

/// Props for [`Terminal`].
#[derive(Props, Clone, PartialEq)]
pub struct TerminalProps {
    pub session: TerminalSession,
    pub on_action: EventHandler<TerminalAction>,
}

/// One pod exec pane: a status line, the scrolled output, and an input row while live.
#[component]
pub fn Terminal(props: TerminalProps) -> Element {
    let session = props.session.clone();
    let action = props.on_action;
    let live = session.is_live();
    let status_text = status_label(&session.status);
    let status_tone = status_tone(&session.status);
    let title = match &session.container {
        Some(container) => format!("{}/{} · {container}", session.namespace, session.pod),
        None => format!("{}/{}", session.namespace, session.pod),
    };

    rsx! {
        section {
            class: "terminal-pane",
            "aria-label": "Pod shell",
            "data-status": "{status_tone}",
            div { class: "terminal-bar",
                span { class: "terminal-title mono", "{title}" }
                span { class: "terminal-status chip {status_tone}", "{status_text}" }
                if session.dropped > 0 {
                    span { class: "chip warn", "data-dropped": "{session.dropped}",
                        "{session.dropped} dropped"
                    }
                }
                button {
                    class: "icon-btn",
                    r#type: "button",
                    "aria-label": "Detach shell",
                    "data-act": "detach",
                    onclick: move |_| action.call(TerminalAction::Detach),
                    "×"
                }
            }
            pre {
                class: "terminal-output mono",
                "aria-live": "polite",
                "{session.text()}"
            }
            if live {
                form {
                    class: "terminal-input-row",
                    onsubmit: move |event: FormEvent| {
                        event.stop_propagation();
                        let line = match event.get_first("line") {
                            Some(FormValue::Text(text)) => text,
                            _ => String::new(),
                        };
                        action.call(TerminalAction::Submit(line));
                    },
                    input {
                        class: "terminal-input mono",
                        name: "line",
                        "aria-label": "Shell input",
                        autocomplete: "off",
                        spellcheck: "false",
                    }
                }
            }
        }
    }
}

fn status_label(status: &TerminalStatus) -> &'static str {
    match status {
        TerminalStatus::Connecting => "Connecting",
        TerminalStatus::Open => "Connected",
        TerminalStatus::Exited(_) => "Exited",
        TerminalStatus::Errored(_) => "Error",
        TerminalStatus::Closed => "Closed",
    }
}

fn status_tone(status: &TerminalStatus) -> &'static str {
    match status {
        TerminalStatus::Connecting => "info",
        TerminalStatus::Open => "ok",
        TerminalStatus::Exited(code) if is_zero_exit(code) => "ok",
        TerminalStatus::Exited(_) => "warn",
        TerminalStatus::Errored(_) => "err",
        TerminalStatus::Closed => "muted",
    }
}

fn is_zero_exit(code: &openkite_api::exec::ExecExitCode) -> bool {
    matches!(code, openkite_api::exec::ExecExitCode::Code(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openkite_api::exec::{ExecEvent, ExecExitCode, ShellChannel};

    /// Handlers must be built inside the render runtime, so the pane is mounted through a
    /// thin harness rather than constructing props in the test body.
    #[derive(Props, Clone, PartialEq)]
    struct HarnessProps {
        session: TerminalSession,
    }

    #[allow(non_snake_case)]
    fn Harness(props: HarnessProps) -> Element {
        let on_action = use_callback(|_: TerminalAction| {});
        rsx! {
            Terminal { session: props.session.clone(), on_action }
        }
    }

    fn render(session: TerminalSession) -> String {
        let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { session });
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn connecting_pane_shows_the_connecting_chip_and_an_input() {
        let html = render(TerminalSession::new("prod", "api-7f", 80, 24));
        assert!(html.contains("Connecting"));
        assert!(html.contains("class=\"terminal-input"));
    }

    #[test]
    fn open_pane_renders_buffered_output() {
        let mut session = TerminalSession::new("prod", "api-7f", 80, 24);
        session.apply_event(ExecEvent::Output {
            channel: ShellChannel::Stdout,
            sequence: 0,
            bytes: b"hello".to_vec(),
        });
        let html = render(session);
        assert!(html.contains("Connected"));
        assert!(html.contains("hello"));
    }

    #[test]
    fn exited_pane_drops_the_input_row() {
        let mut session = TerminalSession::new("prod", "api-7f", 80, 24);
        session.apply_event(ExecEvent::Exit(ExecExitCode::Code(0)));
        let html = render(session);
        assert!(html.contains("Exited"));
        assert!(!html.contains("class=\"terminal-input"));
    }

    #[test]
    fn dropped_frames_surface_as_a_warning_chip() {
        let mut session = TerminalSession::new("prod", "api-7f", 80, 24);
        session.apply_event(ExecEvent::Dropped {
            channel: ShellChannel::Stdout,
            count: 4,
        });
        let html = render(session);
        assert!(html.contains("4 dropped"));
    }
}
