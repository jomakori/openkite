//! The OpenKite host runtime both hosts share: the cluster client and its
//! gateway adapter, the plugin bridge dispatch, the live reflector state, the
//! push channel, and the persisted settings.
//!
//! This is the kube-side half of a host process. The renderer-agnostic console
//! lives in `openkite-ui`; this crate is where a native host (desktop, browser
//! server) keeps the client, the watches and the bridge.

pub mod bridge;
pub mod config;
pub mod gateway;
pub mod push;
pub mod state;
