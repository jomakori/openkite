//! OpenKite web host — the binary half of the browser target.
//!
//! Serves the console bundle built from `web/` and answers the bridge endpoint
//! that bundle calls, so a browser console reads the cluster instead of its
//! bundled fixtures.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use openkite_web::{bind_and_serve, connect, in_cluster, DEFAULT_ADDR, DEFAULT_WEB_ROOT};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let addr: SocketAddr = std::env::var("OPENKITE_ADDR")
        .unwrap_or_else(|_| DEFAULT_ADDR.to_string())
        .parse()
        .context("parse OPENKITE_ADDR")?;
    let web_root = PathBuf::from(
        std::env::var("OPENKITE_WEB_ROOT").unwrap_or_else(|_| DEFAULT_WEB_ROOT.to_string()),
    );

    tracing::info!(
        identity = if in_cluster() {
            "in-cluster"
        } else {
            "kubeconfig"
        },
        "connecting to the cluster"
    );
    let client = connect().await?;
    bind_and_serve(addr, web_root, client).await
}
