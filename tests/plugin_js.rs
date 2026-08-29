//! Integration tests for the JS plugin host.

use openkite::plugin_js::{
    coalesce, discover_plugins, load_manifest, scan_and_reconcile, JsPluginRegistry, PluginAction,
    PluginChange, PluginManifest,
};
use std::fs;
use tempfile::tempdir;

fn manifest(name: &str, version: &str) -> PluginManifest {
    PluginManifest {
        name: name.into(),
        version: version.into(),
        entry: "main.js".into(),
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
    // Non-dirs are ignored.
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
    // Seed: keep + gone (both enabled).
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

    // Version bump = Changed, and enabled state survives.
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
