//! OpenKite web host — the binary half of the browser target.
//!
//! Server-renders the console from the shared UI crate and answers its kube
//! bridge over HTTP, so a browser console reads the cluster instead of
//! bundled fixtures.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use openkite_web::{bind_and_serve, connect, identity_label, DEFAULT_ADDR, DEFAULT_WEB_ROOT};

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

    // OKT-178: name the effective identity (SA namespace or kubeconfig
    // context), not just the in-cluster boolean guess — the ticket's
    // acceptance requires the identity to be named, not assumed.
    tracing::info!(identity = %identity_label(), "connecting to the cluster");
    let client = connect().await?;
    bind_and_serve(addr, web_root, client).await
}
