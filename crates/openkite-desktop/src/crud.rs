//! Resource CRUD: the contract lives in `openkite_api::crud`, the kube-side
//! dispatch in the host's gateway adapter.
//!
//! Re-exported under the app's own path so callers and tests keep one import
//! (`openkite::crud::{Mutation, apply_mutation, …}`).

pub use openkite_api::crud::*;
pub use openkite_host::gateway::apply_mutation;
