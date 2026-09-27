//! The host gateway: the typed async request/response the shared UI dispatches
//! through. One trait, implemented in-process by the desktop host over kube-rs
//! and server-side by the browser host (OKT-134 pass 3).

use std::future::Future;
use std::pin::Pin;

use crate::capability::Capabilities;
use crate::crud::Mutation;
use crate::secret::SecretObject;

/// A boxed future, so [`Gateway`] stays object-safe behind `Arc<dyn Gateway>`.
pub type GatewayFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A gateway failure carrying the host's message verbatim, so the toast copy
/// the console shows is the host's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayError(String);

impl GatewayError {
    /// Wrap a host message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// The message, as the host wrote it.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GatewayError {}

impl From<String> for GatewayError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

/// What the UI may ask the host to do. Every method is `&self` and returns a
/// `'static`-safe boxed future, so a host can install one instance in a
/// process-wide slot and the UI can call it from any task.
pub trait Gateway: Send + Sync + 'static {
    /// What this host can actually do; the console gates on it (OKT-126).
    fn capabilities(&self) -> Capabilities;

    /// Apply one manifest mutation against the cluster.
    fn apply(&self, mutation: Mutation) -> GatewayFuture<'_, Result<(), GatewayError>>;

    /// One secret with the values the slide-over masks.
    fn secret(
        &self,
        namespace: String,
        name: String,
    ) -> GatewayFuture<'_, Result<SecretObject, GatewayError>>;
}
