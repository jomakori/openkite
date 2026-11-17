//! Renders the manifest editor in its four states to a static page, so a browser can
//! screenshot it. Fixtures only; no cluster traffic.
//!
//! Usage: `cargo run -p openkite-ui --example dump_manifest_edit -- <out.html> [clean|lint|diff|conflict]`

use dioxus::prelude::*;
use openkite_ui::manifest_edit::ManifestEdit;
use openkite_ui::manifest_edit_view::ManifestEditor;
use serde_json::Value;

const LIVE: &str = r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
  namespace: openkite
spec:
  replicas: 2
  selector:
    matchLabels:
      app: api
  template:
    spec:
      containers:
        - name: api
          image: ghcr.io/jomakori/api:1.4.0
          env:
            - name: LOG_LEVEL
              value: info
"#;

fn value(text: &str) -> Value {
    serde_yaml_ng::from_str(text).expect("fixture parses")
}

fn fixture(mode: &str) -> (String, ManifestEdit) {
    match mode {
        // Untouched manifest: parses clean and there is nothing to send.
        "clean" => {
            let mut edit = ManifestEdit::start(LIVE);
            edit.edit(LIVE.to_string());
            ("Deployment api".into(), edit)
        }
        // Parser error: the gutter marks the line and the issue list names it.
        "lint" => {
            let broken = LIVE.replace("  replicas: 2\n", "  replicas: 2\n  tolerations: [\n");
            let mut edit = ManifestEdit::start(LIVE);
            edit.edit(broken);
            ("Deployment api".into(), edit)
        }
        // Dry run succeeded with field changes, so apply is armed and the diff renders.
        "diff" => {
            let edited = LIVE
                .replace("replicas: 2", "replicas: 3")
                .replace("value: info", "value: debug");
            let mut edit = ManifestEdit::start(LIVE);
            edit.edit(edited.clone());
            edit.dry_ran(&value(LIVE), &value(&edited));
            ("Deployment api".into(), edit)
        }
        // The server rejected the write because someone else moved the object.
        "conflict" => {
            let submitted = LIVE.replace("replicas: 2", "replicas: 5");
            let moved = LIVE.replace("replicas: 2", "replicas: 4");
            let mut edit = ManifestEdit::start(LIVE);
            edit.edit(submitted.clone());
            edit.conflicted(
                "the object has been modified; apply your change to the latest version",
                &submitted,
                &moved,
            );
            ("Deployment api".into(), edit)
        }
        other => panic!("unknown mode {other}, want clean|lint|diff|conflict"),
    }
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    title: String,
    edit: ManifestEdit,
}

#[component]
fn Harness(props: HarnessProps) -> Element {
    rsx! {
        div { class: "theme-scope", "data-theme": "dark",
            div { class: "dlg-scrim",
                ManifestEditor {
                    title: props.title.clone(),
                    edit: props.edit.clone(),
                    on_input: move |_text: String| {},
                    on_dry_run: move |_| {},
                    on_apply: move |_| {},
                    on_reset: move |_| {},
                    on_close: move |_| {},
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "manifest-edit.html".to_string());
    let mode = args.get(2).map(String::as_str).unwrap_or("diff");
    let (title, edit) = fixture(mode);

    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { title, edit });
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    // The real shell inlines the icon sprite; the dump has to do the same or <use> draws nothing.
    let sprite = include_str!("../assets/icons.svg");
    let page = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>openkite · manifest</title>\
         <link rel=\"stylesheet\" href=\"{}/assets/console.css\"></head>\
         <body class=\"console\">{sprite}{html}</body></html>",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::write(&out, page).expect("write dump");
    println!("{out}");
}
