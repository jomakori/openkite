//! Integration tests for the JS plugin host.

use openkite::plugin_js::{
    coalesce, collect_bundles, discover_plugins, load_manifest, load_source, scan_and_reconcile,
    scope_css, JsBundle, JsPluginRegistry, PluginAction, PluginChange, PluginManifest,
};
use std::fs;
use tempfile::tempdir;

fn manifest(name: &str, version: &str) -> PluginManifest {
    PluginManifest {
        name: name.into(),
        version: version.into(),
        entry: "main.js".into(),
        style: None,
        description: String::new(),
        author: String::new(),
        sidebar: Vec::new(),
    }
}

fn write_plugin(dir: &std::path::Path, name: &str, version: &str) {
    let d = dir.join(name);
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("manifest.json"),
        format!(r#"{{"name":"{name}","version":"{version}","entry":"main.js"}}"#),
    )
    .unwrap();
    fs::write(d.join("main.js"), "// noop").unwrap();
}

/// Like `write_plugin`, but also declares and writes a `style.css`.
fn write_plugin_with_style(dir: &std::path::Path, name: &str, version: &str, css: &str) {
    let d = dir.join(name);
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("manifest.json"),
        format!(
            r#"{{"name":"{name}","version":"{version}","entry":"main.js","style":"style.css"}}"#
        ),
    )
    .unwrap();
    fs::write(d.join("main.js"), "// noop").unwrap();
    fs::write(d.join("style.css"), css).unwrap();
}

#[test]
fn validate_accepts_a_well_formed_manifest() {
    assert!(manifest("argocd", "0.1.0").validate().is_ok());
}

#[test]
fn validate_rejects_bad_names() {
    let mut m = manifest("argocd", "0.1.0");
    m.name = String::new();
    assert!(m.validate().unwrap_err().contains("name"));
    m.name = "bad name/../x".into();
    assert!(m.validate().unwrap_err().contains("only contain"));
}

#[test]
fn validate_rejects_non_js_or_escaping_entries() {
    let mut m = manifest("a", "1.0.0");
    m.entry = "main.ts".into();
    assert!(m.validate().unwrap_err().contains(".js"));
    m.entry = "../evil.js".into();
    assert!(m.validate().unwrap_err().contains("relative"));
    m.entry = "/etc/passwd.js".into();
    assert!(m.validate().unwrap_err().contains("relative"));
}

#[test]
fn validate_rejects_empty_version_and_entry() {
    let m = manifest("a", "");
    assert!(m.validate().unwrap_err().contains("version"));
    let mut m = manifest("a", "1.0.0");
    m.entry = "".into();
    assert!(m.validate().unwrap_err().contains("entry"));
}

#[test]
fn load_manifest_round_trips_and_reports_parse_errors() {
    let dir = tempdir().unwrap();
    write_plugin(dir.path(), "argocd", "0.1.0");
    let loaded = load_manifest(&dir.path().join("argocd")).unwrap();
    assert_eq!(loaded.name, "argocd");
    assert_eq!(loaded.version, "0.1.0");
    assert_eq!(loaded.entry, "main.js");

    let bad = dir.path().join("broken");
    fs::create_dir_all(&bad).unwrap();
    fs::write(bad.join("manifest.json"), "{not json").unwrap();
    assert!(load_manifest(&bad).unwrap_err().contains("parse"));
}

#[test]
fn discover_loads_valid_plugins_and_reports_broken_ones() {
    let dir = tempdir().unwrap();
    write_plugin(dir.path(), "b-plugin", "0.2.0");
    write_plugin(dir.path(), "a-plugin", "0.1.0");
    let broken = dir.path().join("broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("manifest.json"), "{not json").unwrap();
    fs::write(dir.path().join("notes.txt"), "hi").unwrap();

    let (loaded, errors) = discover_plugins(dir.path());
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].0, "a-plugin");
    assert_eq!(loaded[1].0, "b-plugin");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("broken"));
}

#[test]
fn discover_on_missing_root_reports_an_error_not_a_panic() {
    let (loaded, errors) = discover_plugins(std::path::Path::new("/nonexistent/okt45"));
    assert!(loaded.is_empty());
    assert!(!errors.is_empty());
}

#[test]
fn registry_tracks_enabled_state() {
    let mut reg = JsPluginRegistry::new();
    assert!(reg.is_empty());
    reg.upsert(manifest("a", "0.1.0"), true);
    assert!(!reg.is_empty());
    assert!(reg.is_enabled("a"));
    assert!(reg.set_enabled("a", false));
    assert!(!reg.is_enabled("a"));
    assert!(!reg.set_enabled("missing", true));
    assert!(reg.remove("a"));
    assert!(!reg.remove("a"));
    assert!(reg.is_empty());
}

#[test]
fn reconcile_detects_added_removed_and_changed() {
    let dir = tempdir().unwrap();
    write_plugin(dir.path(), "keep", "1.0.0");
    write_plugin(dir.path(), "add", "0.1.0");
    let mut reg = JsPluginRegistry::new();
    reg.upsert(manifest("keep", "1.0.0"), true);
    reg.upsert(manifest("gone", "9.9.9"), true);

    let changes = scan_and_reconcile(dir.path(), &mut reg, true).0;
    assert_eq!(changes.len(), 2);
    assert_eq!(
        changes[0],
        PluginChange {
            name: "add".into(),
            action: PluginAction::Added
        }
    );
    assert_eq!(
        changes[1],
        PluginChange {
            name: "gone".into(),
            action: PluginAction::Removed
        }
    );
    assert!(reg.is_enabled("add"));
    assert!(reg.is_enabled("keep"));

    write_plugin(dir.path(), "keep", "1.1.0");
    let changes = scan_and_reconcile(dir.path(), &mut reg, false).0;
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0],
        PluginChange {
            name: "keep".into(),
            action: PluginAction::Changed
        }
    );
    assert!(
        reg.is_enabled("keep"),
        "enabled state must survive a reload"
    );
}

#[test]
fn reconcile_is_idempotent_without_changes() {
    let dir = tempdir().unwrap();
    write_plugin(dir.path(), "a", "0.1.0");
    let mut reg = JsPluginRegistry::new();
    let first = scan_and_reconcile(dir.path(), &mut reg, true);
    assert_eq!(first.0.len(), 1);
    let second = scan_and_reconcile(dir.path(), &mut reg, true);
    assert!(second.0.is_empty());
    assert!(second.1.is_empty());
}

#[test]
fn action_from_kind_maps_notify_events() {
    use notify::EventKind;
    assert_eq!(
        openkite::plugin_js::action_from_kind(&EventKind::Create(notify::event::CreateKind::File)),
        Some(PluginAction::Added)
    );
    assert_eq!(
        openkite::plugin_js::action_from_kind(&EventKind::Remove(notify::event::RemoveKind::File)),
        Some(PluginAction::Removed)
    );
    assert_eq!(
        openkite::plugin_js::action_from_kind(&EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Any
        ))),
        Some(PluginAction::Changed)
    );
    assert_eq!(
        openkite::plugin_js::action_from_kind(&EventKind::Access(notify::event::AccessKind::Read)),
        None
    );
}

#[test]
fn coalesce_collapses_bursts_last_wins() {
    let burst = vec![
        PluginChange {
            name: "a".into(),
            action: PluginAction::Added,
        },
        PluginChange {
            name: "b".into(),
            action: PluginAction::Added,
        },
        PluginChange {
            name: "a".into(),
            action: PluginAction::Changed,
        },
        PluginChange {
            name: "b".into(),
            action: PluginAction::Removed,
        },
    ];
    let out = coalesce(burst);
    assert_eq!(out.len(), 2);
    assert_eq!(
        out[0],
        PluginChange {
            name: "a".into(),
            action: PluginAction::Changed
        }
    );
    assert_eq!(
        out[1],
        PluginChange {
            name: "b".into(),
            action: PluginAction::Removed
        }
    );
}

#[test]
fn load_manifest_read_error_is_reported() {
    let dir = tempdir().unwrap();
    let err = load_manifest(&dir.path().join("ghost")).unwrap_err();
    assert!(err.contains("read"));
}

#[test]
fn registry_lists_sorted_names_and_len() {
    let mut reg = JsPluginRegistry::new();
    reg.upsert(manifest("b", "0.1.0"), true);
    reg.upsert(manifest("a", "0.1.0"), false);
    assert_eq!(reg.len(), 2);
    assert_eq!(reg.names(), vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn collect_bundles_keeps_only_enabled_plugins() {
    let dir = tempdir().unwrap();
    write_plugin(dir.path(), "keep", "0.1.0");
    write_plugin(dir.path(), "skip", "0.2.0");

    let (bundles, errors) = collect_bundles(dir.path(), |name| name == "keep");
    assert!(errors.is_empty());
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].name, "keep");
    assert_eq!(bundles[0].entry, dir.path().join("keep").join("main.js"));
}

#[test]
fn load_source_reads_bundle_text() {
    let dir = tempdir().unwrap();
    write_plugin(dir.path(), "a", "0.1.0");
    let bundle = JsBundle {
        name: "a".into(),
        entry: dir.path().join("a").join("main.js"),
        style: None,
    };
    assert_eq!(load_source(&bundle).unwrap(), "// noop");
}

#[test]
fn load_source_reports_missing_entry() {
    let bundle = JsBundle {
        name: "a".into(),
        entry: std::path::PathBuf::from("/nonexistent/a/main.js"),
        style: None,
    };
    assert!(load_source(&bundle).unwrap_err().contains("read"));
}

// --- CSS isolation -------------------------------------------------------
//
// `scope_css_scopes_a_plain_selector_under_the_plugin_root` is the isolation
// proof: it fails the moment `scope_rules`/`scope_selector_list` stop
// prefixing selectors with the plugin's `[data-plugin="…"]` root, which is
// exactly what "no scoping" looks like. `two_plugins_declaring_same_class_
// get_distinct_scoped_rules` is the collision proof: it fails if two
// plugins' same-named class ever renders identically (i.e. unscoped).

#[test]
fn scope_css_scopes_a_plain_selector_under_the_plugin_root() {
    let out = scope_css(".panel { color: red; }", "plugin-a").unwrap();
    assert!(out.contains("[data-plugin=\"plugin-a\"] .panel"));
    assert!(!out.trim_start().starts_with(".panel"));
}

#[test]
fn scope_css_scopes_a_comma_separated_selector_list() {
    let out = scope_css(".panel, .widget { color: red; }", "plugin-a").unwrap();
    assert!(out.contains("[data-plugin=\"plugin-a\"] .panel"));
    assert!(out.contains("[data-plugin=\"plugin-a\"] .widget"));
}

#[test]
fn two_plugins_declaring_same_class_get_distinct_scoped_rules() {
    let a = scope_css(".panel { color: red; }", "plugin-a").unwrap();
    let b = scope_css(".panel { color: blue; }", "plugin-b").unwrap();
    assert_ne!(a, b);
    assert!(a.contains("[data-plugin=\"plugin-a\"] .panel"));
    assert!(b.contains("[data-plugin=\"plugin-b\"] .panel"));
    assert!(!a.contains("plugin-b"));
    assert!(!b.contains("plugin-a"));
}

#[test]
fn scope_css_recurses_into_media_supports_and_container_queries() {
    let out = scope_css(
        "@media (min-width: 600px) { .panel { color: red; } }",
        "plugin-a",
    )
    .unwrap();
    assert!(out.contains("@media (min-width: 600px)"));
    assert!(out.contains("[data-plugin=\"plugin-a\"] .panel"));

    let out = scope_css(
        "@supports (display: grid) { .panel { display: grid; } }",
        "plugin-a",
    )
    .unwrap();
    assert!(out.contains("@supports"));
    assert!(out.contains("[data-plugin=\"plugin-a\"] .panel"));

    let out = scope_css(
        "@container (min-width: 200px) { .panel { color: red; } }",
        "plugin-a",
    )
    .unwrap();
    assert!(out.contains("@container"));
    assert!(out.contains("[data-plugin=\"plugin-a\"] .panel"));
}

#[test]
fn scope_css_maps_root_custom_properties_onto_the_plugin_root() {
    let out = scope_css(":root { --accent: red; }", "plugin-a").unwrap();
    assert!(out.contains("[data-plugin=\"plugin-a\"] {"));
}

#[test]
fn scope_css_rejects_font_face_and_keyframes_with_a_reason() {
    let err = scope_css(
        "@font-face { font-family: X; src: url(x.woff); }",
        "plugin-a",
    )
    .unwrap_err();
    assert!(err.contains("font-face"));
    assert!(err.contains("global"));

    let err = scope_css("@keyframes spin { from { opacity: 0; } }", "plugin-a").unwrap_err();
    assert!(err.contains("keyframes"));
    assert!(err.contains("global"));
}

#[test]
fn scope_css_rejects_import_and_host_document_selectors() {
    assert!(scope_css("@import \"other.css\";", "plugin-a")
        .unwrap_err()
        .contains("@import"));
    assert!(scope_css("body { margin: 0; }", "plugin-a")
        .unwrap_err()
        .contains("host document"));
}

#[test]
fn collect_bundles_scopes_a_plugins_declared_style() {
    let dir = tempdir().unwrap();
    write_plugin_with_style(dir.path(), "a", "0.1.0", ".panel { color: red; }");

    let (bundles, errors) = collect_bundles(dir.path(), |_| true);
    assert!(errors.is_empty());
    assert_eq!(bundles.len(), 1);
    let style = bundles[0].style.as_ref().expect("style present");
    assert!(style.contains("[data-plugin=\"a\"] .panel"));
}

#[test]
fn collect_bundles_disables_plugin_whose_css_cannot_be_scoped_but_keeps_siblings() {
    let dir = tempdir().unwrap();
    write_plugin_with_style(dir.path(), "bad", "0.1.0", "@import \"evil.css\";");
    write_plugin(dir.path(), "good", "0.1.0");

    let (bundles, errors) = collect_bundles(dir.path(), |_| true);
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].name, "good");
    assert!(errors
        .iter()
        .any(|e| e.contains("bad") && e.contains("disabled")));
}
