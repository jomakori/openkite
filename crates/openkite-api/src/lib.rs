//! The OpenKite contract: the owned types, the gateway trait, and the host
//! capability descriptor that both hosts and the shared UI agree on.
//!
//! Nothing here names a `kube` or `k8s-openapi` type: a kube type in the
//! gateway signature would pull the Kubernetes client into the wasm build by
//! type-checking alone. The kube → contract adapter lives in the host.
//!
//! Dependency direction: hosts → ui → api.

pub mod bridge;
pub mod capability;
pub mod crud;
pub mod gateway;
pub mod manifest;
pub mod pod;
pub mod secret;
