# Security

OpenKite handles Kubernetes credentials (kubeconfigs, tokens) — treat it as
security-sensitive.

- **No secrets in code.** Pass credentials via environment variables or a
  secret manager (e.g. Doppler).
- **Reporting a vulnerability:** open a private Plane issue (`OKT-*`) with
  `security` in the title, or contact the maintainer directly. Please do not
  disclose publicly before a fix lands.
- **Plugins run with host privileges:** review third-party plugins before
  enabling them.
