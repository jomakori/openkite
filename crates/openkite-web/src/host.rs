//! The native half of the browser host: the axum process, its cluster client
//! and the listeners both are built on.
//!
//! Nothing in the desktop path changes. The kube client, the reflectors
//! ([`crate::headless`]) and the bridge are the shared host runtime's own —
//! this crate depends on the contract, the console and that runtime, never on
//! the desktop app. The browser host adds the render path
//! ([`crate::ssr`]) in front of the same runtime.
//!
//! Split out of `lib.rs` because the modules here need the kube and axum
//! stacks, which the wasm client does not: the crate has to compile for
//! `wasm32-unknown-unknown` to produce its hydration bundle.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use kube::Client;
use openkite_host::bridge::Bridge;
use tokio::net::TcpListener;

/// Address the host binds unless `OPENKITE_ADDR` overrides it. Matches the port
/// the preview image and its chart target.
pub const DEFAULT_ADDR: &str = "0.0.0.0:8080";

/// Bundle directory the host serves unless `OPENKITE_WEB_ROOT` overrides it,
/// resolved against the process working directory.
pub const DEFAULT_WEB_ROOT: &str = "web/dist";

/// Whether the default client comes from the pod's ServiceAccount token.
///
/// Mirrors the choice `Client::try_default` makes, so the boot log and the
/// console's context label name the same identity the host actually reads the
/// cluster with.
pub fn in_cluster() -> bool {
    std::env::var_os("KUBERNETES_SERVICE_HOST").is_some()
}

/// Connect to the cluster: the in-cluster ServiceAccount token when running as a
/// pod, else the active `KUBECONFIG` context.
pub async fn connect() -> anyhow::Result<Client> {
    Client::try_default()
        .await
        .context("connect to the cluster")
}

/// Bind `addr` and serve until the process stops.
pub async fn bind_and_serve(
    addr: SocketAddr,
    web_root: PathBuf,
    client: Client,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    serve(listener, web_root, client).await
}

/// Boot the host on an already-bound listener: install the bridge, start the
/// reflectors, then serve.
///
/// The listener belongs to the caller so that a port clash fails before the
/// reflectors start, and so a test can bind `127.0.0.1:0` and read the port
/// back.
pub async fn serve(listener: TcpListener, web_root: PathBuf, client: Client) -> anyhow::Result<()> {
    let bridge = Arc::new(Bridge::connected(client.clone()));

    let started = crate::headless::start_reflectors(client);
    tracing::info!(started, "reflectors running");

    let addr = listener.local_addr().context("read listener address")?;
    let app = crate::routes::router(bridge, &web_root);
    tracing::info!(
        addr = %addr,
        web_root = %web_root.display(),
        "openkite-web listening"
    );
    axum::serve(listener, app).await.context("serve")
}
