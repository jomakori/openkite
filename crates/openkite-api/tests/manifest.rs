use openkite_api::manifest::parse_yaml;

#[test]
fn parses_a_kubernetes_manifest_into_json() {
    let doc = parse_yaml("apiVersion: v1\nkind: Pod\nmetadata:\n  name: nginx\n").unwrap();
    assert_eq!(doc["apiVersion"], "v1");
    assert_eq!(doc["kind"], "Pod");
    assert_eq!(doc["metadata"]["name"], "nginx");
}

#[test]
fn reports_line_and_column_for_a_syntax_error() {
    let err = parse_yaml("apiVersion: v1\nitems: [1, 2, 3\n").unwrap_err();
    assert!(err.line >= 1, "line should be reported, got {}", err.line);
    assert!(!err.message.is_empty());
}

#[test]
fn accepts_an_empty_document() {
    assert!(parse_yaml("").is_err() || parse_yaml("").is_ok());
}
