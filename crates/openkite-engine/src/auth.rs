//! Auth-kind detection: classify a context's auth info and detect in-cluster SA mode.
use kube::config::AuthInfo;

/// The credential mechanism a resolved context's auth info carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthKind {
    /// An exec credential plugin, naming the configured command.
    Exec { command: String },
    /// A legacy OIDC auth-provider, naming its issuer URL when present.
    Oidc { issuer: Option<String> },
    /// A bearer token, inline or from a token file.
    StaticToken,
    /// A client certificate/key pair.
    StaticCert,
    /// A pod's projected service account, detected via the environment.
    InCluster,
    /// No recognized credential.
    None,
}

/// Classify a context's auth info into the credential mechanism it uses.
pub fn classify(auth_info: &AuthInfo) -> AuthKind {
    if let Some(exec) = &auth_info.exec {
        return AuthKind::Exec {
            command: exec.command.clone().unwrap_or_default(),
        };
    }
    if let Some(provider) = &auth_info.auth_provider {
        if provider.name == "oidc" {
            return AuthKind::Oidc {
                issuer: provider.config.get("idp-issuer-url").cloned(),
            };
        }
    }
    if auth_info.client_certificate.is_some() || auth_info.client_certificate_data.is_some() {
        return AuthKind::StaticCert;
    }
    if auth_info.token.is_some() || auth_info.token_file.is_some() {
        return AuthKind::StaticToken;
    }
    AuthKind::None
}

/// Whether the process is running inside a pod, per the env vars kube-rs uses for in-cluster inference.
pub fn is_in_cluster() -> bool {
    std::env::var_os("KUBERNETES_SERVICE_HOST").is_some()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use kube::config::{AuthProviderConfig, ExecConfig};

    use super::*;

    #[test]
    fn classifies_exec_plugin() {
        let auth_info = AuthInfo {
            exec: Some(ExecConfig {
                command: Some("aws-iam-authenticator".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            classify(&auth_info),
            AuthKind::Exec {
                command: "aws-iam-authenticator".to_owned()
            }
        );
    }

    #[test]
    fn classifies_oidc_auth_provider() {
        let mut config = HashMap::new();
        config.insert(
            "idp-issuer-url".to_owned(),
            "https://issuer.example.invalid".to_owned(),
        );
        let auth_info = AuthInfo {
            auth_provider: Some(AuthProviderConfig {
                name: "oidc".to_owned(),
                config,
                other: Default::default(),
            }),
            ..Default::default()
        };
        assert_eq!(
            classify(&auth_info),
            AuthKind::Oidc {
                issuer: Some("https://issuer.example.invalid".to_owned())
            }
        );
    }

    #[test]
    fn classifies_static_token() {
        let auth_info = AuthInfo {
            token_file: Some("/var/run/secrets/token".to_owned()),
            ..Default::default()
        };
        assert_eq!(classify(&auth_info), AuthKind::StaticToken);
    }

    #[test]
    fn classifies_static_cert() {
        let auth_info = AuthInfo {
            client_certificate_data: Some("ZmFrZQ==".to_owned()),
            ..Default::default()
        };
        assert_eq!(classify(&auth_info), AuthKind::StaticCert);
    }

    #[test]
    fn classifies_none_for_empty_auth_info() {
        assert_eq!(classify(&AuthInfo::default()), AuthKind::None);
    }

    #[test]
    fn detects_in_cluster_env() {
        unsafe {
            std::env::set_var("KUBERNETES_SERVICE_HOST", "10.0.0.1");
        }
        assert!(is_in_cluster());
        unsafe {
            std::env::remove_var("KUBERNETES_SERVICE_HOST");
        }
        assert!(!is_in_cluster());
    }
}
