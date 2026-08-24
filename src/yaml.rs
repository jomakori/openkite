//! YAML parsing with line/column diagnostics for the manifest editor.

use serde_json::Value;

/// A YAML parse error with a 1-based line/column location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub line: u64,
    pub column: u64,
    pub message: String,
}

/// Parse a YAML document into a JSON value, or report the first syntax error
/// with its line/column location.
///
/// YAML is a superset of JSON, so a Kubernetes manifest (maps, lists, strings,
/// numbers, booleans) round-trips cleanly through `serde_json::Value`.
pub fn parse_yaml(text: &str) -> Result<Value, Diagnostic> {
    serde_saphyr::from_str::<Value>(text).map_err(|err| {
        let (line, column) = err
            .location()
            .map(|loc| (loc.line(), loc.column()))
            .unwrap_or((0, 0));
        Diagnostic {
            line,
            column,
            message: err.to_string(),
        }
    })
}
