//! Effective cluster identity (OKT-178): naming what credential the host
//! actually authenticates with, instead of inferring it after the fact.
//!
//! `host::in_cluster()` (desktop + web hosts) only checks for
//! `KUBERNETES_SERVICE_HOST`; it says whether the process *looks* like a
//! pod, not which ServiceAccount it runs as, and it can't tell a healthy
//! in-cluster pod from one whose token mount is missing. [`describe`] reads
//! the same namespace file `kube::Client::try_default` relies on, so the
//! identity can be *named* per the ticket's acceptance text instead of
//! assumed from an env var.

use std::path::Path;

/// Default location of the namespace file kube-rs reads for an in-cluster
/// client (same projected volume as the ServiceAccount token).
const NAMESPACE_FILE: &str = "/var/run/secrets/kubernetes.io/serviceaccount/namespace";

/// The credential source a connected client is using.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterIdentity {
    /// The pod's ServiceAccount, read from the projected token mount.
    ServiceAccount { namespace: String },
    /// A kubeconfig context (desktop app / remote client).
    Kubeconfig { context: String },
}

impl std::fmt::Display for ClusterIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServiceAccount { namespace } => {
                write!(f, "in-cluster ServiceAccount (namespace {namespace})")
            }
            Self::Kubeconfig { context } => write!(f, "kubeconfig context {context}"),
        }
    }
}

/// Path kube-rs reads for the in-cluster ServiceAccount token (same mount as
/// [`NAMESPACE_FILE`], different file in the same projected volume).
const TOKEN_FILE: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";

/// Diagnose the two failure modes the ticket's "Verified" section found —
/// a pod with no projected ServiceAccount token mounted, and a `KUBECONFIG`
/// pointed at a file the process cannot read — before `Client::try_default`
/// runs and turns both into the same opaque "You must be logged in to the
/// server" error.
///
/// `Ok(())` means neither obvious failure mode was found; it does not mean
/// the cluster is reachable, only that the credential itself looks sane.
pub fn preflight() -> Result<(), String> {
    preflight_from(Path::new(TOKEN_FILE))
}

/// Testable core of [`preflight`]: the token path is a parameter so tests
/// don't depend on `/var/run/secrets` existing in CI.
fn preflight_from(token_file: &Path) -> Result<(), String> {
    let in_cluster_env = std::env::var_os("KUBERNETES_SERVICE_HOST").is_some();
    if in_cluster_env && std::fs::read(token_file).is_err() {
        return Err(format!(
            "KUBERNETES_SERVICE_HOST is set but no projected ServiceAccount token is \
             mounted at {}; the pod's ServiceAccount volume is missing or not mounted \
             (a GitOps manifest fix, not a code fix)",
            token_file.display()
        ));
    }
    if let Some(path) = std::env::var_os("KUBECONFIG") {
        if let Err(err) = std::fs::read_to_string(&path) {
            return Err(format!(
                "KUBECONFIG is set to {path:?} but unreadable ({err}); a set-and-unreadable \
                 KUBECONFIG is not an acceptable state — fix its permissions or unset it to \
                 use the in-cluster identity explicitly"
            ));
        }
    }
    Ok(())
}

/// Name the identity the host will authenticate with, without connecting.
///
/// Reads the same signal `kube::Client::try_default` would use first: the
/// in-cluster namespace file. Falling back to the active kubeconfig context
/// when that file is absent. Returns `Err` when neither credential can be
/// located — callers should treat that as the connection failure it is,
/// not render fixture data silently.
pub fn describe() -> anyhow::Result<ClusterIdentity> {
    describe_from(Path::new(NAMESPACE_FILE))
}

/// Testable core of [`describe`]: the namespace file path is a parameter so
/// tests don't depend on `/var/run/secrets` existing in CI.
fn describe_from(namespace_file: &Path) -> anyhow::Result<ClusterIdentity> {
    if let Ok(namespace) = std::fs::read_to_string(namespace_file) {
        return Ok(ClusterIdentity::ServiceAccount {
            namespace: namespace.trim().to_string(),
        });
    }
    let kubeconfig = kube::config::Kubeconfig::read()
        .map_err(|err| anyhow::anyhow!("no in-cluster namespace file and no kubeconfig: {err}"))?;
    let context = kubeconfig
        .current_context
        .ok_or_else(|| anyhow::anyhow!("kubeconfig has no current-context set"))?;
    Ok(ClusterIdentity::Kubeconfig { context })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_service_account_namespace_when_the_token_mount_exists() {
        let dir = std::env::temp_dir().join(format!("okt178-identity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("namespace");
        std::fs::write(&path, "openkite-prod\n").unwrap();

        let identity = describe_from(&path).unwrap();
        assert_eq!(
            identity,
            ClusterIdentity::ServiceAccount {
                namespace: "openkite-prod".to_string()
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn falls_back_to_a_kubeconfig_error_when_no_namespace_file_exists() {
        // Hermetic: point KUBECONFIG at a path that can't parse as one, so
        // this doesn't depend on whether the CI/dev box running the test
        // happens to have a real kubeconfig on disk (it may — this repo's
        // own worktrees do). Serialized with the sibling KUBECONFIG test via
        // `kubeconfig_env_lock` since env vars are process-global.
        let _guard = kubeconfig_env_lock().lock().unwrap();
        let missing = Path::new("/nonexistent/okt178-namespace-probe");
        let prior = std::env::var_os("KUBECONFIG");
        std::env::set_var("KUBECONFIG", "/nonexistent/okt178-kubeconfig-probe");

        let result = describe_from(missing);

        match prior {
            Some(value) => std::env::set_var("KUBECONFIG", value),
            None => std::env::remove_var("KUBECONFIG"),
        }
        assert!(
            result.is_err(),
            "no SA token and no readable kubeconfig must be a named failure, not a silent None"
        );
    }

    /// Serializes tests that mutate the process-global `KUBECONFIG` env var.
    fn kubeconfig_env_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    #[test]
    fn preflight_errs_when_in_cluster_but_token_file_is_missing() {
        let _guard = kubeconfig_env_lock().lock().unwrap();
        let prior_host = std::env::var_os("KUBERNETES_SERVICE_HOST");
        let prior_kubeconfig = std::env::var_os("KUBECONFIG");
        std::env::set_var("KUBERNETES_SERVICE_HOST", "10.0.0.1");
        std::env::remove_var("KUBECONFIG");
        let missing_token = Path::new("/nonexistent/okt178-token-probe");

        let result = preflight_from(missing_token);

        match prior_host {
            Some(value) => std::env::set_var("KUBERNETES_SERVICE_HOST", value),
            None => std::env::remove_var("KUBERNETES_SERVICE_HOST"),
        }
        match prior_kubeconfig {
            Some(value) => std::env::set_var("KUBECONFIG", value),
            None => std::env::remove_var("KUBECONFIG"),
        }

        let err = result.expect_err("no projected token mount must be a named failure");
        assert!(
            err.contains("no projected ServiceAccount token"),
            "wrong message: {err}"
        );
    }

    #[test]
    fn preflight_errs_when_kubeconfig_is_set_and_unreadable() {
        let _guard = kubeconfig_env_lock().lock().unwrap();
        let prior_host = std::env::var_os("KUBERNETES_SERVICE_HOST");
        let prior_kubeconfig = std::env::var_os("KUBECONFIG");
        std::env::remove_var("KUBERNETES_SERVICE_HOST");
        std::env::set_var("KUBECONFIG", "/nonexistent/okt178-kubeconfig-probe");
        let irrelevant_token = Path::new("/nonexistent/okt178-token-probe-2");

        let result = preflight_from(irrelevant_token);

        match prior_host {
            Some(value) => std::env::set_var("KUBERNETES_SERVICE_HOST", value),
            None => std::env::remove_var("KUBERNETES_SERVICE_HOST"),
        }
        match prior_kubeconfig {
            Some(value) => std::env::set_var("KUBECONFIG", value),
            None => std::env::remove_var("KUBECONFIG"),
        }

        let err = result.expect_err("a set-and-unreadable KUBECONFIG must be a named failure");
        assert!(err.contains("KUBECONFIG is set"), "wrong message: {err}");
    }

    #[test]
    fn preflight_ok_when_token_mounted_and_no_kubeconfig_override() {
        let _guard = kubeconfig_env_lock().lock().unwrap();
        let prior_host = std::env::var_os("KUBERNETES_SERVICE_HOST");
        let prior_kubeconfig = std::env::var_os("KUBECONFIG");
        let dir = std::env::temp_dir().join(format!("okt178-preflight-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let token = dir.join("token");
        std::fs::write(&token, "fake-token").unwrap();
        std::env::set_var("KUBERNETES_SERVICE_HOST", "10.0.0.1");
        std::env::remove_var("KUBECONFIG");

        let result = preflight_from(&token);

        match prior_host {
            Some(value) => std::env::set_var("KUBERNETES_SERVICE_HOST", value),
            None => std::env::remove_var("KUBERNETES_SERVICE_HOST"),
        }
        match prior_kubeconfig {
            Some(value) => std::env::set_var("KUBECONFIG", value),
            None => std::env::remove_var("KUBECONFIG"),
        }
        let _ = std::fs::remove_dir_all(&dir);

        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }

    #[test]
    fn service_account_identity_names_the_namespace_in_its_display_form() {
        let identity = ClusterIdentity::ServiceAccount {
            namespace: "openkite-prod".into(),
        };
        assert_eq!(
            identity.to_string(),
            "in-cluster ServiceAccount (namespace openkite-prod)"
        );
    }

    #[test]
    fn kubeconfig_identity_names_the_context_in_its_display_form() {
        let identity = ClusterIdentity::Kubeconfig {
            context: "staging".into(),
        };
        assert_eq!(identity.to_string(), "kubeconfig context staging");
    }
}
