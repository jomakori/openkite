//! Integration tests for app configuration.

use openkite::config::OpenKiteConfig;

#[test]
fn save_load_round_trip() {
    let dir = std::env::temp_dir().join(format!("openkite-config-{}", std::process::id()));
    let path = dir.join("config.toml");

    let config = OpenKiteConfig {
        enabled_plugins: vec!["argocd".into()],
        disabled_plugins: vec!["legacy".into()],
        theme: Some("Tokyo Night".into()),
        font_size: Some(14),
        metrics_enabled: false,
    };
    config.save_to(&path).expect("save");

    let loaded = OpenKiteConfig::load_from(&path);
    assert_eq!(loaded, config);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_loads_default() {
    let missing = std::env::temp_dir().join(format!("openkite-missing-{}", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    assert_eq!(
        OpenKiteConfig::load_from(&missing),
        OpenKiteConfig::default()
    );
}

#[test]
fn default_enables_metrics_and_omits_optional() {
    let d = OpenKiteConfig::default();
    assert!(d.metrics_enabled);
    assert!(d.theme.is_none());
    assert!(d.font_size.is_none());
}

#[test]
fn old_config_without_new_fields_still_loads() {
    let dir = std::env::temp_dir().join(format!("openkite-oldcfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "enabled_plugins = [\"argocd\"]\ndisabled_plugins = []\n",
    )
    .unwrap();

    let loaded = OpenKiteConfig::load_from(&path);
    assert_eq!(loaded.enabled_plugins, vec!["argocd".to_string()]);
    assert!(loaded.theme.is_none());
    assert!(loaded.metrics_enabled); // defaulted to true

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn is_enabled_semantics() {
    let empty = OpenKiteConfig::default();
    assert!(empty.is_enabled("anything")); // empty allowlist = all enabled

    let allow = OpenKiteConfig {
        enabled_plugins: vec!["argocd".into()],
        ..Default::default()
    };
    assert!(allow.is_enabled("argocd"));
    assert!(!allow.is_enabled("other"));

    let disabled_wins = OpenKiteConfig {
        enabled_plugins: vec!["argocd".into()],
        disabled_plugins: vec!["argocd".into()],
        ..Default::default()
    };
    assert!(!disabled_wins.is_enabled("argocd"));
}
