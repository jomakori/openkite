//! PromQL client: instant + range queries against a Prometheus HTTP API.
//!
//! Parses the `{"status":"success","data":{"resultType":"matrix", …}}`
//! response into typed results. The HTTP layer is a thin wrapper over
//! `kube::Client::send`; the JSON parsers are the testable surface.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use http_body_util::BodyExt;
/// A single PromQL instant query.
#[derive(Debug, Clone, PartialEq)]
pub struct PromQuery {
    pub expr: String,
    /// Unix seconds; `None` = server-time (omit `time=`).
    pub time: Option<f64>,
}

/// A single PromQL range query.
#[derive(Debug, Clone, PartialEq)]
pub struct PromRangeQuery {
    pub expr: String,
    pub start: f64, // unix seconds
    pub end: f64,   // unix seconds
    pub step: f64,  // seconds
}

/// One `(timestamp_unix, value)` pair. Timestamp is seconds (Prometheus
/// returns fractional seconds for sub-second steps).
pub type PromSample = (f64, f64);

/// One time series: a metric label set + an ordered sample list.
#[derive(Debug, Clone, PartialEq)]
pub struct PromSeries {
    pub metric: BTreeMap<String, String>,
    pub values: Vec<PromSample>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PromError {
    /// HTTP request returned non-2xx.
    Http(u16, String),
    /// JSON parse failed.
    Parse(String),
    /// Prometheus returned `"status": "error"` with a non-empty `errorType`.
    Query(String),
    /// Prometheus returned success with an empty `result` array.
    Empty,
}

impl std::fmt::Display for PromError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromError::Http(code, body) => write!(f, "prometheus http {code}: {body}"),
            PromError::Parse(msg) => write!(f, "prometheus json parse: {msg}"),
            PromError::Query(msg) => write!(f, "prometheus query error: {msg}"),
            PromError::Empty => write!(f, "prometheus returned no series"),
        }
    }
}

impl std::error::Error for PromError {}

/// Build a PromQL expression that returns the per-pod CPU usage rate
/// (cores). Uses `rate(container_cpu_usage_seconds_total[2m])`.
pub fn build_pod_cpu_expr(namespace: &str, pod: &str) -> String {
    format!(
        r#"sum(rate(container_cpu_usage_seconds_total{{namespace="{namespace}",pod="{pod}",container!=""}}[2m]))"#,
    )
}

/// Build a PromQL expression that returns the per-pod working-set
/// memory in bytes. Uses `container_memory_working_set_bytes`.
pub fn build_pod_memory_expr(namespace: &str, pod: &str) -> String {
    format!(
        r#"sum(container_memory_working_set_bytes{{namespace="{namespace}",pod="{pod}",container!=""}}) by (pod)"#,
    )
}

/// Compose the in-cluster Prometheus URL from the detected service name
/// (the value in `PROMETHEUS`). Default port 9090; HTTPS = no.
pub fn prometheus_base_url(service: &str, namespace: Option<&str>) -> String {
    format!(
        "http://{}.{}.svc.cluster.local:9090",
        service,
        namespace.unwrap_or("monitoring"),
    )
}

/// Current unix time in seconds (floating-point, fractional). Convenience
/// for callers that want a `now` matching Prometheus's `time` param.
pub fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// URL-encode a PromQL expression into `query=…`. Keeps the surface area
/// tiny (the chars Prometheus cares about are all encodeable as
/// percent-escapes).
pub fn url_encode_query(expr: &str) -> String {
    // Minimal encode: spaces, quotes, braces, brackets, commas — the chars
    // a PromQL expression carries that an HTTP query string would otherwise
    // mangle.
    let mut out = String::with_capacity(expr.len());
    for byte in expr.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Compose the full `query` (instant) URL for a Prometheus endpoint.
pub fn instant_url(base: &str, expr: &str, time: Option<f64>) -> String {
    let mut url = format!("{}/api/v1/query?query={}", base, url_encode_query(expr));
    if let Some(t) = time {
        url.push_str(&format!("&time={t}"));
    }
    url
}

/// Compose the full `query_range` URL for a Prometheus endpoint.
pub fn range_url(base: &str, expr: &str, start: f64, end: f64, step: f64) -> String {
    format!(
        "{}/api/v1/query_range?query={}&start={}&end={}&step={}",
        base,
        url_encode_query(expr),
        start,
        end,
        step,
    )
}

/// Parse the JSON `{"status":"success","data":{"resultType":"vector",…}}`
/// body. Public for tests + JS-bridge reuse.
pub fn parse_instant_response(json: &serde_json::Value) -> Result<PromSample, PromError> {
    let status = json
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PromError::Parse("missing `status` field".into()))?;
    if status == "error" {
        let msg = json
            .get("error")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("errorType").and_then(|v| v.as_str()))
            .unwrap_or("unknown")
            .to_string();
        return Err(PromError::Query(msg));
    }
    if status != "success" {
        return Err(PromError::Parse(format!("unknown status `{status}`")));
    }
    let data = json
        .get("data")
        .ok_or_else(|| PromError::Parse("missing `data` field".into()))?;
    let result = data
        .get("result")
        .and_then(|v| v.as_array())
        .ok_or_else(|| PromError::Parse("`data.result` is not an array".into()))?;
    if result.is_empty() {
        return Err(PromError::Empty);
    }
    let first = &result[0];
    let value_pair = first
        .get("value")
        .and_then(|v| v.as_array())
        .ok_or_else(|| PromError::Parse("missing `value` array".into()))?;
    if value_pair.len() < 2 {
        return Err(PromError::Parse("`value` array too short".into()));
    }
    let ts = value_pair[0]
        .as_f64()
        .ok_or_else(|| PromError::Parse("timestamp is not a number".into()))?;
    let value_str = value_pair[1]
        .as_str()
        .ok_or_else(|| PromError::Parse("value is not a string".into()))?;
    let value = parse_value_str(value_str);
    Ok((ts, value))
}

/// Parse the JSON `{"status":"success","data":{"resultType":"matrix",…}}` body.
pub fn parse_range_response(json: &serde_json::Value) -> Result<Vec<PromSeries>, PromError> {
    let status = json
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PromError::Parse("missing `status` field".into()))?;
    if status == "error" {
        let msg = json
            .get("error")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("errorType").and_then(|v| v.as_str()))
            .unwrap_or("unknown")
            .to_string();
        return Err(PromError::Query(msg));
    }
    if status != "success" {
        return Err(PromError::Parse(format!("unknown status `{status}`")));
    }
    let data = json
        .get("data")
        .ok_or_else(|| PromError::Parse("missing `data` field".into()))?;
    let result = data
        .get("result")
        .and_then(|v| v.as_array())
        .ok_or_else(|| PromError::Parse("`data.result` is not an array".into()))?;
    let mut series: Vec<PromSeries> = Vec::with_capacity(result.len());
    for entry in result {
        let metric_obj = entry
            .get("metric")
            .and_then(|v| v.as_object())
            .ok_or_else(|| PromError::Parse("missing `metric` object".into()))?;
        let metric: BTreeMap<String, String> = metric_obj
            .iter()
            .map(|(k, v)| {
                let value = v.as_str().unwrap_or("").to_string();
                (k.clone(), value)
            })
            .collect();
        let values = entry
            .get("values")
            .and_then(|v| v.as_array())
            .ok_or_else(|| PromError::Parse("missing `values` array".into()))?;
        let mut samples: Vec<PromSample> = values
            .iter()
            .filter_map(|pair| {
                let arr = pair.as_array()?;
                if arr.len() < 2 {
                    return None;
                }
                let ts = arr[0].as_f64()?;
                let value_str = arr[1].as_str()?;
                Some((ts, parse_value_str(value_str)))
            })
            .collect();
        // Stable order by timestamp: a `rate()`-driven range can come back
        // out of order, and the chart wants a monotone x-axis.
        samples.sort_by(|a, b| a.0.total_cmp(&b.0));
        series.push(PromSeries {
            metric,
            values: samples,
        });
    }
    Ok(series)
}

/// Parse a Prometheus value string into `f64`. Maps the special tokens
/// `"NaN"`, `"+Inf"`, `"-Inf"` to the matching `f64` constants; everything
/// else goes through `f64::from_str` and falls back to `f64::NAN` on a
/// parse failure (counter-reset nonsense ends up as a flat midline in the
/// sparkline renderer).
fn parse_value_str(raw: &str) -> f64 {
    match raw {
        "NaN" => f64::NAN,
        "+Inf" | "Inf" => f64::INFINITY,
        "-Inf" => f64::NEG_INFINITY,
        other => other.parse().unwrap_or(f64::NAN),
    }
}

/// `GET {base}/api/v1/query?query=…[&time=…]` via `kube::Client`.
///
/// The cluster's bearer token is reused from the active kube context —
/// this is why we go through the in-cluster client instead of a separate
/// `reqwest::Client`. The response is text: the JSON parsers do the
/// typed work.
pub async fn query_instant(
    client: &kube::Client,
    base: &str,
    q: &PromQuery,
) -> Result<PromSample, PromError> {
    let url = instant_url(base, &q.expr, q.time);
    let request = http::Request::builder()
        .method(http::Method::GET)
        .uri(&url)
        .body(kube::client::Body::empty())
        .map_err(|e| PromError::Parse(format!("build request: {e}")))?;
    let response = client
        .send(request)
        .await
        .map_err(|e| PromError::Http(0, format!("send: {e}")))?;
    let status = response.status();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| PromError::Parse(format!("body collect: {e}")))?;
    let text = String::from_utf8(body_bytes.to_bytes().to_vec())
        .map_err(|e| PromError::Parse(format!("body utf8: {e}")))?;
    if !status.is_success() {
        return Err(PromError::Http(status.as_u16(), text));
    }
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| PromError::Parse(format!("json: {e}")))?;
    parse_instant_response(&json)
}

/// `GET {base}/api/v1/query_range?query=…&start=…&end=…&step=…` via `kube::Client`.
pub async fn query_range(
    client: &kube::Client,
    base: &str,
    q: &PromRangeQuery,
) -> Result<Vec<PromSeries>, PromError> {
    let url = range_url(base, &q.expr, q.start, q.end, q.step);
    let request = http::Request::builder()
        .method(http::Method::GET)
        .uri(&url)
        .body(kube::client::Body::empty())
        .map_err(|e| PromError::Parse(format!("build request: {e}")))?;
    let response = client
        .send(request)
        .await
        .map_err(|e| PromError::Http(0, format!("send: {e}")))?;
    let status = response.status();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| PromError::Parse(format!("body collect: {e}")))?;
    let text = String::from_utf8(body_bytes.to_bytes().to_vec())
        .map_err(|e| PromError::Parse(format!("body utf8: {e}")))?;
    if !status.is_success() {
        return Err(PromError::Http(status.as_u16(), text));
    }
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| PromError::Parse(format!("json: {e}")))?;
    parse_range_response(&json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_pod_cpu_expr_quotes_namespace_and_pod() {
        let expr = build_pod_cpu_expr("default", "coredns-abc");
        assert!(expr.contains("container_cpu_usage_seconds_total"));
        assert!(expr.contains(r#"namespace="default""#));
        assert!(expr.contains(r#"pod="coredns-abc""#));
    }

    #[test]
    fn build_pod_memory_expr_uses_working_set_bytes() {
        let expr = build_pod_memory_expr("default", "coredns-abc");
        assert!(expr.contains("container_memory_working_set_bytes"));
        assert!(expr.contains(r#"namespace="default""#));
        assert!(expr.contains(r#"pod="coredns-abc""#));
    }

    #[test]
    fn url_encode_query_escapes_spaces_and_quotes() {
        let encoded = url_encode_query("up{job=\"prometheus\"} and {foo=\"bar\"}");
        assert!(encoded.contains("%20"));
        assert!(encoded.contains("%22"));
    }

    #[test]
    fn url_encode_query_passes_unreserved_chars() {
        let encoded = url_encode_query("abc-DEF_1.2~3");
        assert_eq!(encoded, "abc-DEF_1.2~3");
    }

    #[test]
    fn instant_url_omits_time_when_none() {
        let url = instant_url("http://prom:9090", "up", None);
        assert!(!url.contains("time="));
    }

    #[test]
    fn instant_url_includes_time_when_some() {
        let url = instant_url("http://prom:9090", "up", Some(1700000000.5));
        assert!(url.contains("time=1700000000.5"));
    }

    #[test]
    fn range_url_formats_all_three_params() {
        let url = range_url("http://prom:9090", "up", 1.0, 60.0, 5.0);
        assert!(url.contains("start=1"));
        assert!(url.contains("end=60"));
        assert!(url.contains("step=5"));
    }

    #[test]
    fn prometheus_base_url_formats_cluster_local() {
        assert_eq!(
            prometheus_base_url("prometheus-operated", Some("monitoring")),
            "http://prometheus-operated.monitoring.svc.cluster.local:9090"
        );
    }

    #[test]
    fn prometheus_base_url_defaults_to_monitoring_namespace() {
        assert_eq!(
            prometheus_base_url("prometheus", None),
            "http://prometheus.monitoring.svc.cluster.local:9090"
        );
    }

    #[test]
    fn parse_instant_response_extracts_value() {
        let json: serde_json::Value = serde_json::json!({
            "status": "success",
            "data": {
                "resultType": "vector",
                "result": [
                    {"metric": {"pod": "x"}, "value": [1700000000, "0.42"]}
                ]
            }
        });
        let (ts, value) = parse_instant_response(&json).unwrap();
        assert_eq!(ts, 1700000000.0);
        assert!((value - 0.42).abs() < 1e-9);
    }

    #[test]
    fn parse_instant_response_handles_nan_value_string() {
        let json: serde_json::Value = serde_json::json!({
            "status": "success",
            "data": {
                "resultType": "vector",
                "result": [
                    {"metric": {"pod": "x"}, "value": [1700000000, "NaN"]}
                ]
            }
        });
        let (_, value) = parse_instant_response(&json).unwrap();
        assert!(value.is_nan());
    }

    #[test]
    fn parse_instant_response_handles_infinities() {
        let json: serde_json::Value = serde_json::json!({
            "status": "success",
            "data": {
                "resultType": "vector",
                "result": [
                    {"metric": {"x": "y"}, "value": [1700000000, "+Inf"]},
                    {"metric": {"x": "z"}, "value": [1700000001, "-Inf"]}
                ]
            }
        });
        let (_, v0) = parse_instant_response(&json).unwrap();
        assert!(v0.is_infinite() && v0 > 0.0);
    }

    #[test]
    fn parse_instant_response_empty_result_is_empty_error() {
        let json: serde_json::Value = serde_json::json!({
            "status": "success",
            "data": {"resultType": "vector", "result": []}
        });
        assert!(matches!(
            parse_instant_response(&json),
            Err(PromError::Empty)
        ));
    }

    #[test]
    fn parse_instant_response_query_error_maps_to_query_variant() {
        let json: serde_json::Value = serde_json::json!({
            "status": "error",
            "errorType": "bad_data",
            "error": "invalid expression"
        });
        match parse_instant_response(&json) {
            Err(PromError::Query(msg)) => assert!(msg.contains("invalid")),
            other => panic!("expected Query error, got {other:?}"),
        }
    }

    #[test]
    fn parse_range_response_extracts_all_samples() {
        let json: serde_json::Value = serde_json::json!({
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {
                        "metric": {"pod": "x", "namespace": "default"},
                        "values": [
                            [1700000000, "0.1"],
                            [1700000030, "0.2"]
                        ]
                    }
                ]
            }
        });
        let series = parse_range_response(&json).unwrap();
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].metric.get("pod").map(String::as_str), Some("x"));
        assert_eq!(
            series[0].metric.get("namespace").map(String::as_str),
            Some("default")
        );
        assert_eq!(series[0].values.len(), 2);
        assert!((series[0].values[0].1 - 0.1).abs() < 1e-9);
        assert!((series[0].values[1].1 - 0.2).abs() < 1e-9);
    }

    #[test]
    fn parse_range_response_orders_samples_by_timestamp() {
        let json: serde_json::Value = serde_json::json!({
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {
                        "metric": {"pod": "x"},
                        "values": [
                            [1700000060, "0.3"],
                            [1700000000, "0.1"],
                            [1700000030, "0.2"]
                        ]
                    }
                ]
            }
        });
        let series = parse_range_response(&json).unwrap();
        let ts: Vec<f64> = series[0].values.iter().map(|(t, _)| *t).collect();
        assert_eq!(ts, vec![1700000000.0, 1700000030.0, 1700000060.0]);
    }

    #[test]
    fn parse_range_response_empty_result_is_empty_vec() {
        let json: serde_json::Value = serde_json::json!({
            "status": "success",
            "data": {"resultType": "matrix", "result": []}
        });
        let series = parse_range_response(&json).unwrap();
        assert!(series.is_empty());
    }

    #[test]
    fn parse_range_response_multiple_series_each_carry_metric() {
        let json: serde_json::Value = serde_json::json!({
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {"metric": {"pod": "a"}, "values": [[1700000000, "0.1"]]},
                    {"metric": {"pod": "b"}, "values": [[1700000000, "0.2"]]}
                ]
            }
        });
        let series = parse_range_response(&json).unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].metric.get("pod").map(String::as_str), Some("a"));
        assert_eq!(series[1].metric.get("pod").map(String::as_str), Some("b"));
    }
}
