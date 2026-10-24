//! The desktop half of gate G3: `OPENKITE_PARITY_SNAPSHOT` boots the app onto
//! one core route, waits for layout, dumps the shared parity snapshot and
//! exits.
//!
//! Opt-in by environment only — with the variable unset this module does
//! nothing and costs one `env::var` at startup. `e2e/parity/run.sh` drives it
//! once per route under Xvfb, alongside the browser extractor, and
//! `e2e/parity/compare.mjs` grades the two halves against each other.
//!
//! ```text
//! OPENKITE_PARITY_SNAPSHOT=<dir> OPENKITE_ROUTE=/workloads ./openkite
//!   → <dir>/workloads.desktop.json
//! ```

use std::path::Path;
use std::time::Duration;

use dioxus::prelude::*;

/// Environment variable naming the directory the snapshots are written to.
pub const SNAPSHOT_DIR_ENV: &str = "OPENKITE_PARITY_SNAPSHOT";

/// Environment variable shortening the post-mount settle (milliseconds).
pub const SETTLE_ENV: &str = "OPENKITE_PARITY_SETTLE_MS";

/// Install the snapshot hook on the app root. A no-op unless the gate asked
/// for a snapshot; never runs in a normal launch.
pub fn install() {
    let Ok(dir) = std::env::var(SNAPSHOT_DIR_ENV) else {
        return;
    };
    let route = std::env::var("OPENKITE_ROUTE").unwrap_or_else(|_| "/".to_string());
    let Some(path) = openkite_ui::parity::snapshot_path(Path::new(&dir), &route, "desktop") else {
        tracing::error!(
            route = %route,
            "OPENKITE_PARITY_SNAPSHOT set for a route that is not graded; no snapshot written"
        );
        std::process::exit(2);
    };
    let settle = openkite_ui::parity::settle_ms(std::env::var(SETTLE_ENV).ok().as_deref());
    tracing::info!(path = %path.display(), settle_ms = settle, "parity snapshot requested");

    use_effect(move || {
        let path = path.clone();
        spawn(async move {
            // One settled frame after mount, so computed values are final.
            tokio::time::sleep(Duration::from_millis(settle)).await;
            // Bounded on purpose: a renderer that never answers must fail the
            // gate loudly instead of hanging the route until the harness kills
            // it with no explanation.
            let received = tokio::time::timeout(
                Duration::from_secs(30),
                document::eval(openkite_ui::parity::SNAPSHOT_JS).recv::<String>(),
            )
            .await;
            match received {
                Ok(Ok(json)) => {
                    if let Err(error) = std::fs::write(&path, json) {
                        tracing::error!(error = %error, path = %path.display(), "parity snapshot write failed");
                        std::process::exit(2);
                    }
                    tracing::info!(path = %path.display(), "parity snapshot written");
                    std::process::exit(0);
                }
                Ok(Err(error)) => {
                    tracing::error!(error = ?error, "parity snapshot evaluation failed");
                    std::process::exit(2);
                }
                Err(_) => {
                    tracing::error!("parity snapshot evaluation timed out after 30s");
                    std::process::exit(2);
                }
            }
        });
    });
}
