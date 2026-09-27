//! The release version, resolved at runtime (OKT-107).
//!
//! A released artifact may have been built before its version existed (a reused
//! PR package), so the compile-time value is the fallback, never the source.

/// The variable a release or deployment injects the version with.
pub const ENV_VAR: &str = "OPENKITE_VERSION";

/// The AppImage runtime exports the path of the bundle it is running.
const APPIMAGE_VAR: &str = "APPIMAGE";

/// The workspace placeholder: a build carrying it has no release version.
pub const PLACEHOLDER: &str = "0.0.0";

const COMPILED: &str = env!("CARGO_PKG_VERSION");

/// The version to report: the injected one, else the version the release wrote
/// into the running AppImage's asset name, else the compile-time one. `None`
/// means the artifact carries no release version.
pub fn reported() -> Option<String> {
    injected(std::env::var(ENV_VAR).ok().as_deref())
        .or_else(|| asset_named(std::env::var(APPIMAGE_VAR).ok().as_deref()))
        .or_else(|| injected(Some(COMPILED)))
}

fn injected(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty() && *candidate != PLACEHOLDER)
        .map(str::to_string)
}

fn asset_named(path: Option<&str>) -> Option<String> {
    let file = path?.rsplit('/').next()?;
    injected(file.strip_prefix("openkite_")?.split('_').next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_value_wins_and_blank_or_placeholder_falls_through() {
        assert_eq!(injected(Some(" 1.2.3 ")).as_deref(), Some("1.2.3"));
        assert_eq!(injected(Some("")), None);
        assert_eq!(injected(Some("   ")), None);
        assert_eq!(injected(Some(PLACEHOLDER)), None);
        assert_eq!(injected(None), None);
    }

    #[test]
    fn release_asset_name_carries_the_version() {
        assert_eq!(
            asset_named(Some("/opt/openkite_1.2.3_linux_amd64.AppImage")).as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            asset_named(Some("/opt/openkite_0.0.0_linux_amd64.AppImage")),
            None
        );
        assert_eq!(asset_named(Some("/opt/OpenKite.AppImage")), None);
        assert_eq!(asset_named(Some("/opt/openkite.AppImage")), None);
        assert_eq!(asset_named(None), None);
    }

    #[test]
    fn reported_reads_the_injected_variable_and_never_the_placeholder() {
        std::env::set_var(ENV_VAR, "9.9.9");
        let injected = super::reported();
        std::env::set_var(ENV_VAR, PLACEHOLDER);
        let placeholder = super::reported();
        std::env::remove_var(ENV_VAR);
        assert_eq!(injected.as_deref(), Some("9.9.9"));
        assert_ne!(placeholder.as_deref(), Some(PLACEHOLDER));
    }
}
