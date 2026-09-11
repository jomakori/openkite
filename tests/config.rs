//! Integration tests for app configuration.

use openkite::config::{MenuBarVisibility, OpenKiteConfig};

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
        menu_bar: MenuBarVisibility::Hide,
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
fn menu_bar_defaults_to_show_and_persists_as_camel_case() {
    assert_eq!(OpenKiteConfig::default().menu_bar, MenuBarVisibility::Show);

    let dir = std::env::temp_dir().join(format!("openkite-menubar-{}", std::process::id()));
    let path = dir.join("config.toml");
    let config = OpenKiteConfig {
        menu_bar: MenuBarVisibility::Hide,
        ..Default::default()
    };
    config.save_to(&path).expect("save");
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("menuBar = \"hide\""), "got: {raw}");

    // An old file without the key loads as `show`.
    std::fs::write(&path, "enabled_plugins = []\ndisabled_plugins = []\n").unwrap();
    assert_eq!(
        OpenKiteConfig::load_from(&path).menu_bar,
        MenuBarVisibility::Show
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn menu_bar_visibility_toggles_and_reports_visibility() {
    assert_eq!(MenuBarVisibility::Show.toggled(), MenuBarVisibility::Hide);
    assert_eq!(MenuBarVisibility::Hide.toggled(), MenuBarVisibility::Show);
    assert!(MenuBarVisibility::Show.is_visible());
    assert!(!MenuBarVisibility::Hide.is_visible());
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
    // `metrics_enabled` is unset in the fixture; the default is true.
    assert!(loaded.metrics_enabled);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn is_enabled_semantics() {
    let empty = OpenKiteConfig::default();
    // An empty allowlist enables every plugin.
    assert!(empty.is_enabled("anything"));

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

#[test]
fn corrupt_toml_falls_back_to_default() {
    let dir = std::env::temp_dir().join(format!("openkite-corrupt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "enabled_plugins = [unterminated").unwrap();

    assert_eq!(OpenKiteConfig::load_from(&path), OpenKiteConfig::default());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_to_errors_when_parent_cannot_be_created() {
    let dir = std::env::temp_dir().join(format!("openkite-blocked-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A regular file where the parent directory should be.
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, "not a directory").unwrap();

    assert!(OpenKiteConfig::default()
        .save_to(&blocker.join("config.toml"))
        .is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_and_load_use_the_home_config_path() {
    let dir = std::env::temp_dir().join(format!("openkite-home-{}", std::process::id()));
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    // Point HOME at the sandbox so `load()`/`save()` hit a known path.
    std::env::set_var("HOME", &home);

    let config = OpenKiteConfig {
        enabled_plugins: vec!["argocd".into()],
        disabled_plugins: vec![],
        theme: Some("catppuccin-mocha".into()),
        font_size: Some(13),
        metrics_enabled: false,
        menu_bar: MenuBarVisibility::Show,
    };
    config
        .save()
        .expect("save() persists to ~/.openkite/config.toml");
    assert_eq!(OpenKiteConfig::load(), config);
    assert!(home.join(".openkite").join("config.toml").exists());

    let _ = std::fs::remove_dir_all(&dir);
}
