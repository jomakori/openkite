//! Integration tests for fuzzy matching.

use openkite::fuzzy::{fuzzy_match, rank};

#[test]
fn exact_match_beats_partial() {
    let exact = fuzzy_match("pod", "pod").unwrap();
    let partial = fuzzy_match("pod", "pod-log-viewer").unwrap();
    assert!(exact.score >= partial.score);
}

#[test]
fn subsequence_records_positions() {
    let m = fuzzy_match("pdl", "pod-deploy-logs").unwrap();
    assert_eq!(m.positions, vec![0, 2, 7]);
}

#[test]
fn non_subsequence_is_none() {
    assert!(fuzzy_match("xyz", "pod").is_none());
}

#[test]
fn matching_is_case_insensitive() {
    assert!(fuzzy_match("POD", "pod").is_some());
    assert!(fuzzy_match("pod", "POD").is_some());
}

#[test]
fn consecutive_beats_gapped() {
    let consecutive = fuzzy_match("ab", "abc").unwrap();
    let gapped = fuzzy_match("ab", "a-b").unwrap();
    assert!(consecutive.score > gapped.score);
}

#[test]
fn empty_query_matches_everything() {
    assert!(fuzzy_match("", "anything").is_some());
    assert_eq!(rank("", [("pod", 1), ("svc", 2)]).len(), 2);
}

#[test]
fn rank_orders_best_first() {
    let ranked = rank("dp", [("service", 1), ("deploy", 2), ("delete-pod", 3)]);
    // "deploy" (consecutive d..p) outranks "delete-pod" (gapped d..p); "service" drops.
    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].1, 2);
    assert_eq!(ranked[1].1, 3);
}
