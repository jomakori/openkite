//! Integration tests for YAML parsing + diagnostics.

use openkite::yaml::parse_yaml;

#[test]
fn parses_valid_yaml_into_json_value() {
    let v = parse_yaml("apiVersion: v1\nkind: Pod\nreplicas: 3\n").unwrap();
    assert_eq!(v["apiVersion"], "v1");
    assert_eq!(v["kind"], "Pod");
    assert_eq!(v["replicas"], 3);
}

#[test]
fn parses_nested_structures() {
    let v = parse_yaml("spec:\n  containers:\n    - name: app\n      image: nginx\n").unwrap();
    assert_eq!(v["spec"]["containers"][0]["name"], "app");
    assert_eq!(v["spec"]["containers"][0]["image"], "nginx");
}

#[test]
fn reports_error_with_location_on_bad_yaml() {
    let err = parse_yaml("apiVersion: v1\nitems: [1, 2, 3\n").unwrap_err();
    assert!(!err.message.is_empty());
    assert!(err.line >= 1, "line should be reported, got {}", err.line);
}
