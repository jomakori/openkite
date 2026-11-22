//! Fixture-backed data source: boots the seeded fake apiserver in-process and points the
//! engine at it, so a preview with no cluster credential still renders a populated console.

use std::sync::OnceLock;

use openkite_engine::Engine;
use openkite_engine_testkit::{demo, FakeApiserver};

use crate::Live;

/// Name the console shows for a fixture connection, where a kube context name would sit.
pub const CONTEXT: &str = "fixtures";

/// Whether this process should render fixtures instead of a cluster.
pub fn requested() -> bool {
    std::env::var("OPENKITE_DATA_MODE").is_ok_and(|mode| mode.eq_ignore_ascii_case("fixtures"))
}

/// Connects to the seeded cluster. The apiserver lives in a static for the life of the
/// process: dropping it would close the port the client points at.
pub async fn connect() -> Live {
    static APISERVER: OnceLock<FakeApiserver> = OnceLock::new();

    if APISERVER.get().is_none() {
        let _ = APISERVER.set(FakeApiserver::start(demo::scenario()).await);
    }
    let apiserver = APISERVER.get().expect("fixture apiserver was just set");

    tracing::info!(
        base_url = apiserver.base_url(),
        namespace = demo::NAMESPACE,
        "serving fixture data"
    );
    Live::from_engine(Engine::from_client(apiserver.client(), CONTEXT))
}
