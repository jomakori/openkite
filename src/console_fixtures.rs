//! Test-only fixture bridge for the console parity gate (OKT-129).
//!
//! The parity gate renders the *same* console surface twice — once in headless
//! Chromium over the served `web/dist` (a static host, so the console falls
//! back to the fixtures it bundles) and once on this host under Xvfb — and
//! pixel-compares the two. For that comparison to measure *rendering* rather
//! than *data*, both sides must be shown the same payloads. The browser's
//! payloads come from `web/src/fixtures.ts`; this module serves the export of
//! that same module (`e2e/parity/fixtures.json`, written by
//! `npm run export:parity-fixtures`) so the host answers with them too.
//!
//! Gating: [`ENV_VAR`] must name a readable export. Unset (or empty) leaves
//! every production path untouched — this mirrors the already-accepted
//! `OPENKITE_ROUTE` test hook, and the browser host in `crates/openkite-web`
//! shares [`crate::bridge::Bridge`], so it honours the same variable for free.
//!
//! What is deliberately reproduced:
//!
//! - `list` / `watch` — the kind's kube `List`, namespace-filtered, and an
//!   **empty** list for a kind the export does not carry (the browser's
//!   `fixtureList` never throws for a list, so neither may this);
//! - `get` — the object, or the same not-found error text the browser raises;
//! - `subscribe` — `{sub, initial}` from the push registry, which is exactly
//!   what the live path returns and what the JS bootstrap delivers to its
//!   handler as the first snapshot;
//! - `logs` / `exec` — an error, because the browser's fixture provider has no
//!   fixture for them and the console falls back to its own bundled lines. A
//!   host that answered these would render *different* log content in fixture
//!   mode than the browser does;
//! - the `/openkite-spike` `context` and `settings_get` payloads — the cluster
//!   chip, the shell footer version and the applied theme tokens. The browser
//!   resolves these to `FIXTURE_CONTEXT` and `DEFAULT_SETTINGS` when it has no
//!   host transport, and the console paints the context chip and the status
//!   version straight from them, so a host answering its real (disconnected,
//!   empty-theme) values would guarantee a pixel difference.
//!
//! `register` / `unsubscribe` are host bookkeeping, never fixture data, and
//! fall through to their normal handling.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use k8s_openapi::jiff::{SignedDuration, Timestamp};
use serde_json::{json, Value};

use crate::plugin_api::ApiRequest;

/// Names the exported fixture payloads (`e2e/parity/fixtures.json`).
pub const ENV_VAR: &str = "OPENKITE_CONSOLE_FIXTURES";

/// The exported payloads the browser console renders from.
#[derive(Debug, Clone)]
pub struct Fixtures {
    /// `FIXTURE_CONTEXT` (`web/src/fixtures.ts`): the shell's cluster chip.
    context: Value,
    /// `DEFAULT_SETTINGS` (`web/src/settings.ts`), when the export carries it.
    settings: Option<Value>,
    /// kind → kube `List`, rebased onto this process's clock.
    kinds: BTreeMap<String, Value>,
}

impl Fixtures {
    /// Parse an export. Timestamps are shifted from the export's clock onto
    /// this process's, because fixture ages (`12m`) are built from wall-clock
    /// time at payload-build time — a committed copy would otherwise age by
    /// however long it sat unused, and the two sides would show different
    /// ages for the same data.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(text).map_err(|err| format!("parse fixtures: {err}"))?;
        let exported_at = value
            .get("exportedAt")
            .and_then(Value::as_i64)
            .ok_or_else(|| "fixtures: missing `exportedAt`".to_string())?;
        let exported_at = Timestamp::from_millisecond(exported_at)
            .map_err(|err| format!("fixtures: exportedAt: {err}"))?;
        let context = value
            .get("context")
            .cloned()
            .ok_or_else(|| "fixtures: missing `context`".to_string())?;
        let kinds_value = value
            .get("kinds")
            .and_then(Value::as_object)
            .ok_or_else(|| "fixtures: missing `kinds`".to_string())?;

        let delta = SignedDuration::from_millis(
            Timestamp::now().as_millisecond() - exported_at.as_millisecond(),
        );
        let mut kinds = BTreeMap::new();
        for (kind, list) in kinds_value {
            let mut list = list.clone();
            rebase(&mut list, delta);
            kinds.insert(kind.clone(), list);
        }
        Ok(Self {
            context,
            settings: value.get("settings").cloned(),
            kinds,
        })
    }

    /// How many kinds the export carries (logged at activation).
    pub fn kind_count(&self) -> usize {
        self.kinds.len()
    }

    /// `FIXTURE_CONTEXT` verbatim.
    pub fn context(&self) -> Value {
        self.context.clone()
    }

    /// `DEFAULT_SETTINGS` verbatim, when the export carries it.
    pub fn settings(&self) -> Option<Value> {
        self.settings.clone()
    }

    /// Kube `List` for `kind`, filtered to `ns` — the shape the browser's
    /// `fixtureList` returns for the same inputs. A kind the export does not
    /// carry is an empty list, never an error.
    pub fn list(&self, kind: &str, ns: Option<&str>) -> Value {
        let plural = plural_kind(kind);
        let Some(list) = self.kinds.get(&plural) else {
            return json!({
                "apiVersion": "v1",
                "kind": format!("{}List", capitalise(&plural)),
                "items": [],
            });
        };
        let Some(ns) = ns.filter(|ns| !ns.is_empty()) else {
            return list.clone();
        };
        let mut narrowed = list.clone();
        if let Some(items) = narrowed.get_mut("items").and_then(Value::as_array_mut) {
            items.retain(|item| {
                item.pointer("/metadata/namespace")
                    .and_then(Value::as_str)
                    .is_some_and(|item_ns| item_ns == ns)
            });
        }
        narrowed
    }

    /// One object, or the not-found error the browser's `fixtureGet` raises.
    pub fn get(&self, kind: &str, ns: &str, name: &str) -> Result<Value, String> {
        let plural = plural_kind(kind);
        let found = self
            .kinds
            .get(&plural)
            .and_then(|list| list.get("items"))
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    let item_name = item
                        .pointer("/metadata/name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let item_ns = item
                        .pointer("/metadata/namespace")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    item_name == name && item_ns == ns
                })
            });
        found
            .cloned()
            .ok_or_else(|| format!("fixture: {plural}/{ns}/{name} not found"))
    }
}

/// Answer one bridge request from the fixture export, when fixture mode is
/// active. `None` means "not fixture mode" and leaves the caller's normal
/// (live) handling in place.
pub fn answer(request: &ApiRequest) -> Option<Result<Value, String>> {
    active().map(|fixtures| answer_with(fixtures, request))
}

/// The fixture answer for one request (`register`/`unsubscribe` have none).
fn answer_with(fixtures: &Fixtures, request: &ApiRequest) -> Result<Value, String> {
    match request {
        ApiRequest::List { kind, ns } => Ok(fixtures.list(kind, ns.as_deref())),
        // The live `watch` path answers a bare array of rows; mirror it so the
        // payload shape does not depend on which mode served it.
        ApiRequest::Watch { kind, ns } => {
            let list = fixtures.list(kind, ns.as_deref());
            Ok(list
                .get("items")
                .and_then(Value::as_array)
                .map(|items| Value::Array(items.clone()))
                .unwrap_or_else(|| Value::Array(Vec::new())))
        }
        ApiRequest::Get { kind, ns, name } => fixtures.get(kind, ns, name),
        // Allocate a real subscription id so `unsubscribe` stays symmetric,
        // and hand the snapshot back as `initial` — the same `{sub, initial}`
        // the live path returns, which the JS bootstrap delivers to the
        // handler. Fixtures are static, so nothing ever publishes to it.
        ApiRequest::Subscribe { kind, ns } => {
            let sub = match crate::push::registry().lock() {
                Ok(mut registry) => registry.subscribe(kind.clone(), ns.clone()),
                Err(poisoned) => poisoned.into_inner().subscribe(kind.clone(), ns.clone()),
            };
            Ok(json!({ "sub": sub, "initial": fixtures.list(kind, ns.as_deref()) }))
        }
        // No fixture exists for these, and answering them would render
        // different content than the browser (which falls back to its own
        // bundled log lines). The console's own fallback supplies the lines.
        ApiRequest::Logs { .. } => Err(no_fixture("logs")),
        ApiRequest::Exec { .. } => Err(no_fixture("exec")),
        ApiRequest::Register { .. } | ApiRequest::Unsubscribe { .. } => {
            unreachable!("register/unsubscribe never reach the fixture answer")
        }
    }
}

/// `window.openkite` / `fixtureCall` wording for an op with no fixture.
fn no_fixture(op: &str) -> String {
    format!("fixture: op '{op}' has no fixture")
}

/// The cluster context the browser paints when it has no host transport.
pub fn context() -> Option<Value> {
    active().map(Fixtures::context)
}

/// The settings snapshot the browser applies when it has no host transport.
pub fn settings() -> Option<Value> {
    active().and_then(Fixtures::settings)
}

/// Whether fixture mode is live for this process.
pub fn enabled() -> bool {
    active().is_some()
}

/// Emit the boot line (and prime the cache) so a fixture-mode run is visible in
/// `app.log` before the first bridge call. No output when the hook is unset.
pub fn log_activation() {
    let _ = active();
}

/// The parsed export, loaded at most once per process.
fn active() -> Option<&'static Fixtures> {
    static CACHE: OnceLock<Option<Fixtures>> = OnceLock::new();
    CACHE
        .get_or_init(|| load_from(env_path().as_deref()))
        .as_ref()
}

/// The configured export path; empty counts as unset.
fn env_path() -> Option<PathBuf> {
    std::env::var_os(ENV_VAR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Read + parse an export; any failure disables fixture mode (loudly) rather
/// than failing requests.
fn load_from(path: Option<&Path>) -> Option<Fixtures> {
    let path = path?;
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            tracing::error!(path = %path.display(), error = %err, "console fixtures: unreadable export");
            return None;
        }
    };
    match Fixtures::parse(&text) {
        Ok(fixtures) => {
            tracing::info!(
                "console fixtures: enabled ({} kinds from {})",
                fixtures.kind_count(),
                path.display()
            );
            Some(fixtures)
        }
        Err(err) => {
            tracing::error!(path = %path.display(), error = %err, "console fixtures: ignoring malformed export");
            None
        }
    }
}

/// Shift every `creationTimestamp` in a payload by `delta`.
fn rebase(value: &mut Value, delta: SignedDuration) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if key == "creationTimestamp" {
                    if let Some(text) = child.as_str() {
                        if let Some(shifted) = shift(text, delta) {
                            *child = Value::String(shifted);
                        }
                    }
                    continue;
                }
                rebase(child, delta);
            }
        }
        Value::Array(items) => {
            for item in items {
                rebase(item, delta);
            }
        }
        _ => {}
    }
}

/// One RFC 3339 timestamp moved by `delta`; unparseable input is left alone.
fn shift(text: &str, delta: SignedDuration) -> Option<String> {
    let parsed: Timestamp = text.parse().ok()?;
    parsed
        .checked_add(delta)
        .ok()
        .map(|shifted| shifted.to_string())
}

/// Pluralise a bare kind (`pod` → `pods`); kinds already plural pass through —
/// the same rule `web/src/fixtures.ts` uses.
fn plural_kind(kind: &str) -> String {
    let lower = kind.to_lowercase();
    if lower.ends_with('s') {
        lower
    } else {
        format!("{lower}s")
    }
}

/// Upcase the first character (list kinds: `pods` → `PodsList`).
fn capitalise(plural: &str) -> String {
    let mut chars = plural.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny export: two pods in `default`, one in `kube-system`, one
    /// cluster-scoped node, and a timestamp 12 minutes before `exportedAt`.
    fn sample_json() -> String {
        let exported_at = Timestamp::now().as_millisecond();
        let created = Timestamp::now()
            .checked_sub(SignedDuration::from_mins(12))
            .unwrap();
        json!({
            "context": { "context": "staging (fixtures)", "connected": true, "version": "staging" },
            "settings": { "theme": null, "themeVars": {}, "version": "dev", "menuBar": "show" },
            "exportedAt": exported_at,
            "kinds": {
                "pods": {
                    "apiVersion": "v1",
                    "kind": "PodsList",
                    "items": [
                        { "metadata": { "name": "api-0", "namespace": "default", "creationTimestamp": created.to_string() } },
                        { "metadata": { "name": "api-1", "namespace": "default", "creationTimestamp": created.to_string() } },
                        { "metadata": { "name": "dns-0", "namespace": "kube-system", "creationTimestamp": created.to_string() } }
                    ]
                },
                "nodes": {
                    "apiVersion": "v1",
                    "kind": "NodesList",
                    "items": [
                        { "metadata": { "name": "node-0", "creationTimestamp": created.to_string() } }
                    ]
                },
                "configmaps": {
                    "apiVersion": "v1",
                    "kind": "ConfigMapsList",
                    "items": []
                }
            }
        })
        .to_string()
    }

    fn sample() -> Fixtures {
        Fixtures::parse(&sample_json()).expect("sample export parses")
    }

    #[test]
    fn parse_reads_context_kinds_and_the_settings_payload() {
        let fixtures = sample();
        assert_eq!(fixtures.kind_count(), 3);
        assert_eq!(fixtures.context()["context"], "staging (fixtures)");
        assert_eq!(fixtures.settings().unwrap()["version"], "dev");
        assert!(fixtures.settings().unwrap()["themeVars"]
            .as_object()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn parse_rejects_a_payload_without_a_clock_or_context() {
        let no_clock = r#"{"context": {}, "kinds": {}}"#;
        assert!(Fixtures::parse(no_clock)
            .unwrap_err()
            .contains("exportedAt"));
        let no_context = r#"{"exportedAt": 0, "kinds": {}}"#;
        assert!(Fixtures::parse(no_context).unwrap_err().contains("context"));
        let no_kinds = r#"{"exportedAt": 0, "context": {}}"#;
        assert!(Fixtures::parse(no_kinds).unwrap_err().contains("kinds"));
        assert!(Fixtures::parse("not json")
            .unwrap_err()
            .contains("parse fixtures"));
    }

    #[test]
    fn an_absent_or_unreadable_export_disables_fixture_mode() {
        assert!(load_from(None).is_none(), "unset ⇒ no fixture mode");
        assert!(
            load_from(Some(Path::new("/nonexistent/okt129-fixtures.json"))).is_none(),
            "unreadable path ⇒ no fixture mode"
        );
        assert!(env_path().is_none() || std::env::var(ENV_VAR).is_ok());
    }

    #[test]
    fn list_filters_by_namespace_and_unknown_kinds_are_empty_not_errors() {
        let fixtures = sample();
        let all = fixtures.list("pods", None);
        assert_eq!(all["items"].as_array().unwrap().len(), 3);
        assert_eq!(all["kind"], "PodsList");

        let default = fixtures.list("pods", Some("default"));
        assert_eq!(default["items"].as_array().unwrap().len(), 2);

        let empty_ns = fixtures.list("pods", Some(""));
        assert_eq!(
            empty_ns["items"].as_array().unwrap().len(),
            3,
            "empty ns = all"
        );

        // A namespace-scoped view of a cluster-scoped kind keeps nothing —
        // `web/src/fixtures.ts` filters the same way.
        assert_eq!(
            fixtures.list("nodes", Some("default"))["items"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            fixtures.list("nope", None)["items"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(fixtures.list("nope", None)["kind"], "NopesList");
        // Singular kinds pluralise like the browser's provider.
        assert_eq!(
            fixtures.list("pod", None)["items"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn get_returns_the_object_and_misses_match_the_browser_message() {
        let fixtures = sample();
        let pod = fixtures.get("pods", "default", "api-0").unwrap();
        assert_eq!(pod["metadata"]["name"], "api-0");
        assert_eq!(
            fixtures.get("pods", "default", "nope").unwrap_err(),
            "fixture: pods/default/nope not found"
        );
        assert_eq!(
            fixtures.get("pods", "kube-system", "api-0").unwrap_err(),
            "fixture: pods/kube-system/api-0 not found"
        );
    }

    #[test]
    fn timestamps_are_rebased_onto_this_process_clock() {
        let fixtures = sample();
        let item = &fixtures.list("pods", None)["items"][0];
        let stamp = item["metadata"]["creationTimestamp"].as_str().unwrap();
        let created = stamp.parse::<Timestamp>().unwrap();
        // The sample is 12 minutes old; the rebase must preserve that age.
        let minutes = (Timestamp::now().as_millisecond() - created.as_millisecond()) / 60_000;
        assert!(
            (11..=13).contains(&minutes),
            "a 12-minute-old fixture must still read as ~12 minutes, got {minutes}m"
        );
    }

    #[test]
    fn configmap_lists_round_trip_as_an_empty_kube_list() {
        let fixtures = sample();
        let list = fixtures.list("configmaps", None);
        assert_eq!(list["kind"], "ConfigMapsList");
        assert!(list["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn subscribe_answers_sub_and_initial_and_releases_the_subscription() {
        let fixtures = sample();
        let answer = answer_with(
            &fixtures,
            &ApiRequest::Subscribe {
                kind: "pods".into(),
                ns: None,
            },
        )
        .unwrap();
        let sub = answer["sub"].as_u64().unwrap();
        assert!(sub > 0);
        assert_eq!(answer["initial"]["items"].as_array().unwrap().len(), 3);
        let removed = crate::push::registry().lock().unwrap().unsubscribe(sub);
        assert!(
            removed,
            "the fixture answer must allocate a real subscription"
        );
    }

    #[test]
    fn list_watch_and_get_answers_are_served_from_the_export() {
        let fixtures = sample();
        let listed = answer_with(
            &fixtures,
            &ApiRequest::List {
                kind: "pods".into(),
                ns: Some("kube-system".into()),
            },
        )
        .unwrap();
        assert_eq!(listed["items"].as_array().unwrap().len(), 1);

        let watched = answer_with(
            &fixtures,
            &ApiRequest::Watch {
                kind: "pods".into(),
                ns: None,
            },
        )
        .unwrap();
        assert_eq!(watched.as_array().unwrap().len(), 3, "watch answers rows");

        let got = answer_with(
            &fixtures,
            &ApiRequest::Get {
                kind: "pods".into(),
                ns: "default".into(),
                name: "api-1".into(),
            },
        )
        .unwrap();
        assert_eq!(got["metadata"]["name"], "api-1");
    }

    #[test]
    fn logs_and_exec_have_no_fixture_on_the_host_either() {
        let fixtures = sample();
        let logs = answer_with(
            &fixtures,
            &ApiRequest::Logs {
                name: "api-0".into(),
                ns: "default".into(),
                container: None,
            },
        )
        .unwrap_err();
        assert_eq!(logs, "fixture: op 'logs' has no fixture");
        let exec = answer_with(
            &fixtures,
            &ApiRequest::Exec {
                name: "api-0".into(),
                ns: "default".into(),
                container: None,
                cmd: vec!["sh".into()],
            },
        )
        .unwrap_err();
        assert_eq!(exec, "fixture: op 'exec' has no fixture");
    }

    #[test]
    fn settings_are_absent_when_the_export_does_not_carry_them() {
        let text = json!({
            "context": { "context": "staging (fixtures)" },
            "exportedAt": Timestamp::now().as_millisecond(),
            "kinds": {}
        })
        .to_string();
        let fixtures = Fixtures::parse(&text).unwrap();
        assert!(fixtures.settings().is_none());
    }

    #[test]
    fn shift_leaves_unparseable_timestamps_alone() {
        let delta = SignedDuration::from_secs(60);
        assert_eq!(shift("not a timestamp", delta), None);
        assert_eq!(shift("", delta), None);
    }
}
