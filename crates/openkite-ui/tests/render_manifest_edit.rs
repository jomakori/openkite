//! Renders the YAML editor panel through SSR and checks what the reader gets: a number
//! for every line, a marker on the line a diagnostic names, one diff row per changed
//! field, an armed apply only after a dry run, and both versions on a 409.

use dioxus::prelude::*;
use openkite_ui::manifest_edit::{lint, ManifestEdit, Phase};
use openkite_ui::manifest_edit_view::ManifestEditor;
use serde_json::Value;

const LIVE: &str = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: api\n  namespace: openkite\nspec:\n  replicas: 2\n";

fn value(text: &str) -> Value {
    serde_yaml_ng::from_str(text).expect("fixture parses")
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    edit: ManifestEdit,
}

#[component]
fn Harness(props: HarnessProps) -> Element {
    rsx! {
        div { class: "dlg-scrim",
            ManifestEditor {
                title: "Deployment api",
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

fn render(edit: ManifestEdit) -> String {
    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { edit });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn editing(text: &str) -> ManifestEdit {
    let mut edit = ManifestEdit::start(LIVE);
    edit.edit(text);
    edit
}

#[test]
fn the_panel_opens_inside_the_scrim() {
    let html = render(ManifestEdit::start(LIVE));
    let scrim = html.find("dlg-scrim").expect("scrim renders");
    let panel = html.find("yaml-editor").expect("panel renders");
    assert!(
        scrim < panel,
        "the dialog has to sit inside the scrim, not beside it"
    );
}

#[test]
fn the_gutter_numbers_every_line() {
    let html = render(ManifestEdit::start(LIVE));
    assert_eq!(html.matches("class=\"gl\"").count(), LIVE.lines().count());
}

#[test]
fn a_parse_error_marks_the_gutter_and_names_the_issue() {
    let broken = "apiVersion: v1\nkind: Pod\nmetadata: [\n";
    let html = render(editing(broken));
    assert!(
        html.contains("class=\"gl error\""),
        "the offending line carries the error mark"
    );
    assert!(html.contains("yaml-issues"), "the issue list renders");
    assert!(
        html.contains("class=\"yaml-status bad\""),
        "the status pill turns red"
    );
}

#[test]
fn a_clean_parse_leaves_no_marks() {
    let html = render(ManifestEdit::start(LIVE));
    assert_eq!(html.matches("class=\"gl error\"").count(), 0);
    assert_eq!(html.matches("class=\"gl warn\"").count(), 0);
}

#[test]
fn the_diff_table_lists_one_row_per_changed_field() {
    let edited = LIVE.replace("replicas: 2", "replicas: 3");
    let mut edit = editing(&edited);
    edit.dry_ran(&value(LIVE), &value(&edited));
    let html = render(edit);
    assert!(html.contains("yaml-diff"), "the diff block renders");
    assert!(html.contains("spec.replicas"), "the changed path is named");
    assert_eq!(
        html.matches("class=\"diff-row").count(),
        1,
        "one row for one change"
    );
}

#[test]
fn apply_is_disabled_until_a_dry_run_succeeds() {
    let edited = LIVE.replace("replicas: 2", "replicas: 3");

    let before = editing(&edited);
    assert!(!before.can_apply(), "no dry run means nothing to apply");
    assert!(before.can_dry_run(), "but the dry run is available");
    assert!(
        render(before).contains("disabled"),
        "the button renders disabled"
    );

    let mut after = editing(&edited);
    after.dry_ran(&value(LIVE), &value(&edited));
    assert!(after.can_apply(), "a clean dry run arms apply");

    // Typing again drops back to editing, so the diff has to be re-run.
    after.edit(LIVE.replace("replicas: 2", "replicas: 4"));
    assert!(!after.can_apply());
    assert_eq!(after.phase, Phase::Editing);
}

#[test]
fn a_conflict_shows_both_versions() {
    let submitted = LIVE.replace("replicas: 2", "replicas: 5");
    let moved = LIVE.replace("replicas: 2", "replicas: 4");
    let mut edit = editing(&submitted);
    edit.conflicted("the object has been modified", &submitted, &moved);
    let html = render(edit);
    assert!(html.contains("yaml-conflict"), "the pair renders");
    assert!(html.contains("replicas: 5"), "the sent version is shown");
    assert!(html.contains("replicas: 4"), "the live version is shown");
    assert!(
        html.contains("the object has been modified"),
        "the reason is quoted"
    );
}

#[test]
fn a_warning_does_not_block_the_dry_run() {
    // A duplicate key is a warning, not an error, so the text stays sendable.
    let text = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: a\n  name: b\n";
    let result = lint(text);
    assert_eq!(result.errors(), 0);
    assert!(
        result.warnings() > 0,
        "the duplicate is reported: {:?}",
        result.diagnostics
    );
    let edit = editing(text);
    assert!(edit.can_dry_run());
}
