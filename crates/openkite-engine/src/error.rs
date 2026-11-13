//! Typed errors for kubeconfig loading, context resolution, auth, and client construction.
use std::path::PathBuf;

use kube::config::{InClusterError, KubeconfigError, LoadDataError};
use thiserror::Error;

/// All failure modes surfaced by the engine connect seam.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Named context is not present in any loaded kubeconfig.
    #[error("unknown context '{0}'")]
    UnknownContext(String),

    /// A context's cluster reference does not resolve to a defined cluster.
    #[error("context '{context}' references missing cluster '{cluster}'")]
    MissingClusterRef { context: String, cluster: String },

    /// A context's user reference does not resolve to a defined user.
    #[error("context '{context}' references missing user '{user}'")]
    MissingUserRef { context: String, user: String },

    /// A file referenced by the kubeconfig could not be read.
    #[error("unreadable file '{}': {source}", path.display())]
    UnreadableFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// No context was given and no kubeconfig current-context is set.
    #[error("no context given and no current-context is set")]
    CurrentContextNotSet,

    /// Kubeconfig loading, parsing, or merging failed.
    #[error("kubeconfig error: {0}")]
    Kubeconfig(#[from] KubeconfigError),

    /// In-cluster configuration could not be loaded from the pod environment.
    #[error("in-cluster config error: {0}")]
    InCluster(#[from] InClusterError),

    /// Building the kube client from a resolved config failed.
    #[error("client build error: {0}")]
    ClientBuild(#[from] kube::Error),
}

impl EngineError {
    /// Narrow a generic kubeconfig error into `UnreadableFile` when it wraps a file read failure.
    pub(crate) fn from_config_error(err: KubeconfigError) -> Self {
        match err {
            KubeconfigError::ReadConfig(source, path) => {
                EngineError::UnreadableFile { path, source }
            }
            KubeconfigError::LoadCertificateAuthority(LoadDataError::ReadFile(source, path))
            | KubeconfigError::LoadClientCertificate(LoadDataError::ReadFile(source, path))
            | KubeconfigError::LoadClientKey(LoadDataError::ReadFile(source, path)) => {
                EngineError::UnreadableFile { path, source }
            }
            other => EngineError::Kubeconfig(other),
        }
    }
}
