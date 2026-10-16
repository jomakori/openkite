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

/// Directory the wasm hydration bundle is served from unless
/// `OPENKITE_WEB_ROOT` overrides it, resolved against the process working
/// directory. After OKT-137 there is no prebuilt console bundle: the crate
/// server-renders the page, and this root only carries the hydration assets
/// (`openkite-web-client.js` and its `.wasm`).
pub const DEFAULT_WEB_ROOT: &str = "public";

/// Whether the default client comes from the pod's ServiceAccount token.
///
/// Mirrors the choice `Client::try_default` makes, so the boot log and the
/// console's context label name the same identity the host actually reads the
/// cluster with.
pub fn in_cluster() -> bool {
    std::env::var_os("KUBERNETES_SERVICE_HOST").is_some()
}

/// Connect to the cluster: the in-cluster ServiceAccount token when running as a
/// pod, else the active `KUBECONFIG` context. Returns `Ok(None)` when neither
/// credential source is usable at all (OKT-191): a preview/CI host with no
/// cluster credential is a supported, deterministic mode — the caller serves
/// the fixture gateway instead of failing to boot.
///
/// The crypto provider is installed before the client builds its TLS
/// configuration. A build that also builds dev-dependencies resolves both of
/// rustls' providers — kube's `ring` next to rustls' default `aws-lc-rs` — and
/// rustls refuses to choose between two, so the host picks one itself.
///
/// OKT-178: before calling kube-rs at all, run [`openkite_host::identity::preflight`]
/// so the two failure modes the ticket found — a pod with no projected
/// ServiceAccount token, and a `KUBECONFIG` set to a file the process can't
/// read — fail with an actionable message instead of kube-rs's opaque "You
/// must be logged in to the server". That preflight still runs and still
/// errors on those two broken-auth modes; only a genuinely absent credential
/// (not in-cluster, no resolvable kubeconfig) answers `Ok(None)`. The identity
/// itself (SA namespace or kubeconfig context) is logged by the caller via
/// [`identity_label`].
pub async fn connect() -> anyhow::Result<Option<Client>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    openkite_host::identity::preflight().map_err(anyhow::Error::msg)?;
    if !in_cluster() && kube::config::Kubeconfig::read().is_err() {
        return Ok(None);
    }
    Client::try_default()
        .await
        .map(Some)
        .context("connect to the cluster")
}

/// The effective cluster identity, named per OKT-178's acceptance text
/// ("the pod's effective identity is named in the ticket"), for the boot log
/// and the console's context label. Falls back to the raw [`in_cluster`]
/// guess when the identity source can be seen but not read, and names a
/// credential-less start honestly as fixture mode (OKT-191) rather than
/// guessing at an identity that isn't there.
pub fn identity_label() -> String {
    match openkite_host::identity::describe() {
        Ok(identity) => identity.to_string(),
        Err(_) if in_cluster() => "in-cluster (identity unreadable)".to_string(),
        Err(_) if kube::config::Kubeconfig::read().is_ok() => {
            "kubeconfig (identity unreadable)".to_string()
        }
        Err(_) => "no-cluster (fixture)".to_string(),
    }
}

/// Bind `addr` and serve until the process stops.
///
/// `client` is `None` for a credential-less start (OKT-191): the bridge then
/// holds no client and the routes fall back to the fixture gateway instead of
/// failing to boot.
pub async fn bind_and_serve(
    addr: SocketAddr,
    web_root: PathBuf,
    client: Option<Client>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    serve(listener, web_root, client).await
}

/// Boot the host on an already-bound listener: install the bridge, start the
/// reflectors (when there is a client to start them with), then serve.
///
/// The listener belongs to the caller so that a port clash fails before the
/// reflectors start, and so a test can bind `127.0.0.1:0` and read the port
/// back.
pub async fn serve(
    listener: TcpListener,
    web_root: PathBuf,
    client: Option<Client>,
) -> anyhow::Result<()> {
    // Publish host capabilities before any gateway exists (disconnected boot).
    openkite_ui::runtime::set_published_capabilities(Some(
        openkite_api::capability::Capabilities::server_side(),
    ));

    let bridge = Arc::new(Bridge::new());
    bridge.set_client(client.clone());

    let started = match client {
        Some(client) => crate::headless::start_reflectors(client),
        None => 0,
    };
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
