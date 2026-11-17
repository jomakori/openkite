//! Model and render tests for the palette.

use dioxus::prelude::*;
use openkite_ui::palette::{Entry, Hit, Palette, RESULT_LIMIT};
use openkite_ui::palette_view::PaletteOverlay;

fn index() -> Palette {
    Palette::build(vec![
        Entry::new("pod", "prod", "api-7d9c4f8b6-2xqzt").with_labels(&["app=api", "tier=web"]),
        Entry::new("pod", "prod", "worker-6b8f7d5c4-zz2lt").with_labels(&["app=worker"]),
        Entry::new("pod", "kube-system", "coredns-5d78c9869d-8kx7p").with_labels(&["k8s-app=dns"]),
        Entry::new("deployment", "prod", "api").with_labels(&["app=api", "tier=web"]),
        Entry::new("node", "", "ip-10-0-3-17.eu-west-1.compute.internal")
            .with_labels(&["role=worker"]),
        Entry::new("event", "prod", "BackOff api-7d9c4f8b6-2xqzt").with_labels(&["reason=backoff"]),
        Entry::new("namespace", "", "prod").with_labels(&["team=payments"]),
    ])
}

fn names(hits: &[Hit]) -> Vec<String> {
    hits.iter().map(|h| h.title().to_string()).collect()
}

#[test]
fn a_name_prefix_finds_the_object() {
    let hits = index().query("core");
    assert_eq!(names(&hits), vec!["coredns-5d78c9869d-8kx7p"]);
}

#[test]
fn a_token_fragment_matches_every_part_of_a_name() {
    let hits = index().query("api");
    let found = names(&hits);
    assert!(found.contains(&"api".to_string()), "{found:?}");
    assert!(
        found.contains(&"api-7d9c4f8b6-2xqzt".to_string()),
        "{found:?}"
    );
    assert!(
        found.contains(&"BackOff api-7d9c4f8b6-2xqzt".to_string()),
        "{found:?}"
    );
}

#[test]
fn an_exact_name_outranks_a_prefix_match() {
    let hits = index().query("api");
    assert_eq!(hits[0].title(), "api", "{:?}", names(&hits));
}

#[test]
fn labels_are_searchable_as_key_value_pairs() {
    let hits = index().query("app=api");
    let found = names(&hits);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .all(|n| n == "api" || n == "api-7d9c4f8b6-2xqzt"),
        "{found:?}"
    );
    let Hit::Object { label, .. } = &hits[0] else {
        panic!("expected an object")
    };
    assert_eq!(
        label.as_deref(),
        Some("app=api"),
        "the matching label is shown"
    );
}

#[test]
fn a_label_value_fragment_also_matches() {
    let hits = index().query("k8s-app=dns");
    assert_eq!(names(&hits), vec!["coredns-5d78c9869d-8kx7p"]);
}

#[test]
fn a_namespace_narrows_the_result() {
    let prod = index().query("ns=kube-system");
    assert_eq!(names(&prod), vec!["coredns-5d78c9869d-8kx7p"]);

    let both = index().query("ns=prod api");
    let found = names(&both);
    assert!(found.iter().all(|n| !n.contains("coredns")), "{found:?}");
    assert!(
        found.contains(&"api-7d9c4f8b6-2xqzt".to_string()),
        "{found:?}"
    );
}

#[test]
fn a_kind_narrows_the_result() {
    let hits = index().query("kind=pod");
    assert_eq!(hits.len(), 3, "{:?}", names(&hits));
    assert!(hits.iter().all(|h| h.subtitle().starts_with("pod")));
}

#[test]
fn a_bare_namespace_still_filters() {
    let hits = index().query("kube-system");
    assert_eq!(names(&hits), vec!["coredns-5d78c9869d-8kx7p"]);
}

#[test]
fn two_fragments_must_both_match() {
    let hits = index().query("prod worker");
    assert_eq!(names(&hits), vec!["worker-6b8f7d5c4-zz2lt"]);
    assert!(index().query("prod coredns").is_empty());
}

#[test]
fn an_empty_query_lists_commands() {
    let hits = index().query("");
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|h| matches!(h, Hit::Command { .. })));
}

#[test]
fn the_chevron_switches_to_commands_only() {
    let hits = index().query(">logs");
    assert_eq!(hits.len(), 1, "{:?}", names(&hits));
    assert!(
        matches!(hits[0], Hit::Command { id: "logs", .. }),
        "{hits:?}"
    );

    let all = index().query(">");
    assert!(all.len() >= 10, "commands listed: {}", all.len());
    assert!(all.iter().all(|h| matches!(h, Hit::Command { .. })));
}

#[test]
fn objects_and_commands_share_the_result_list() {
    let hits = index().query("log");
    let objects = hits
        .iter()
        .filter(|h| matches!(h, Hit::Object { .. }))
        .count();
    let commands = hits
        .iter()
        .filter(|h| matches!(h, Hit::Command { .. }))
        .count();
    assert_eq!(commands, 1, "the log dock command should match: {hits:?}");
    assert_eq!(objects, 0, "no object is named log: {hits:?}");

    let hits = index().query("over");
    assert!(
        matches!(hits[0], Hit::Command { id: "overview", .. }),
        "{hits:?}"
    );
}

#[test]
fn nothing_matching_returns_nothing() {
    assert!(index().query("zzzznotathing").is_empty());
    assert!(index().query(">zzzz").is_empty());
}

#[test]
fn the_result_limit_is_respected() {
    let many: Vec<Entry> = (0..RESULT_LIMIT * 3)
        .map(|i| Entry::new("pod", "prod", &format!("job-{i}")))
        .collect();
    let palette = Palette::build(many);
    assert_eq!(palette.query("job").len(), RESULT_LIMIT);
}

#[test]
fn hits_carry_a_stable_key() {
    let hits = index().query("api");
    let keys: Vec<String> = hits.iter().map(Hit::key).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), keys.len(), "keys collide: {keys:?}");
    assert!(
        keys.contains(&"pod/prod/api-7d9c4f8b6-2xqzt".to_string()),
        "{keys:?}"
    );
}

// ---- render

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    query: String,
    hits: Vec<Hit>,
    active: usize,
    open: bool,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_query = use_callback(|_: String| {});
    let on_active = use_callback(|_: usize| {});
    let on_accept = use_callback(|_: Hit| {});
    let on_close = use_callback(|_: ()| {});
    rsx! {
        PaletteOverlay {
            open: props.open,
            query: props.query.clone(),
            hits: props.hits.clone(),
            active: props.active,
            on_query,
            on_active,
            on_accept,
            on_close,
        }
    }
}

fn render(query: &str, active: usize) -> String {
    let hits = index().query(query);
    let props = HarnessProps {
        query: query.to_string(),
        hits,
        active,
        open: true,
    };
    let mut dom = VirtualDom::new_with_props(Harness, props);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn a_closed_palette_renders_nothing() {
    let props = HarnessProps {
        query: String::new(),
        hits: Vec::new(),
        active: 0,
        open: false,
    };
    let mut dom = VirtualDom::new_with_props(Harness, props);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(!html.contains("cm-panel"), "{html}");
}

#[test]
fn the_dialog_sits_inside_the_scrim() {
    let html = render("app=api", 0);
    let before = &html[..html.find("cm-panel").expect("panel")];
    // A sibling dialog would leave the scrim closed, so the panel must open at depth 2.
    let depth = before.matches("<div").count() - before.matches("</div>").count();
    assert_eq!(depth, 2, "panel is not nested in the scrim: {html}");
    assert!(
        html.trim_end().ends_with("</div>"),
        "scrim never closes: {html}"
    );
}

#[test]
fn the_overlay_is_a_modal_dialog_with_a_search_field() {
    let html = render("api", 0);
    assert!(html.contains("role=\"dialog\""), "{html}");
    assert!(html.contains("aria-modal=\"true\""), "{html}");
    assert!(html.contains("cm-input"), "{html}");
    assert!(html.contains("dlg-scrim"), "{html}");
    assert!(html.contains("Search objects, or"), "{html}");
}

#[test]
fn hits_are_grouped_objects_then_commands() {
    let html = render("log", 0);
    let objects = html.find(">Objects<");
    let commands = html.find(">Commands<");
    assert!(
        objects.is_none(),
        "no object matched, so no Objects group: {html}"
    );
    assert!(commands.is_some(), "{html}");
    assert!(html.contains("0 objects · 1 commands"), "{html}");
}

#[test]
fn the_active_row_is_marked_and_only_one_is() {
    let html = render("api", 1);
    assert_eq!(html.matches("aria-selected=\"true\"").count(), 1, "{html}");
}

#[test]
fn command_rows_show_their_keys_and_objects_do_not() {
    let commands = render(">pods", 0);
    assert!(commands.contains("cm-key"), "{commands}");
    assert!(commands.contains("g p"), "{commands}");
    let objects = render("worker", 0);
    assert!(!objects.contains("cm-key\">g"), "{objects}");
}

#[test]
fn an_empty_result_says_so() {
    let html = render("zzzznotathing", 0);
    assert!(html.contains("Nothing matches that."), "{html}");
    assert!(html.contains("0 objects · 0 commands"), "{html}");
}

#[test]
fn the_footer_carries_the_keys() {
    let html = render("", 0);
    assert!(html.contains("move"), "{html}");
    assert!(html.contains("open"), "{html}");
    assert!(html.contains("close"), "{html}");
}
