//! Pure-model tests for the ctrl-tab switcher: filtering, ordering, and
//! cursor advancement. UI wiring is exercised in the app.

use openkite::switcher::{advance_index, filter_contexts};

fn names() -> Vec<String> {
    ["dev", "staging-eu", "prod", "prod-us", "sandbox"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn blank_query_returns_all_in_kubeconfig_order() {
    let got = filter_contexts(&names(), "");
    assert_eq!(got, names(), "blank query must preserve kubeconfig order");
}

#[test]
fn whitespace_only_query_returns_all() {
    let got = filter_contexts(&names(), "   \t ");
    assert_eq!(got, names());
}

#[test]
fn filter_is_case_insensitive_substring() {
    let got = filter_contexts(&names(), "PROD");
    assert_eq!(got, vec!["prod".to_string(), "prod-us".to_string()]);
}

#[test]
fn filter_orders_by_match_position() {
    // "us" matches "prod-us" at pos 5 and "staging-eu"… not at all; both
    // prod entries match at pos 0, stable for ties.
    let got = filter_contexts(&names(), "us");
    assert_eq!(got, vec!["prod".to_string(), "prod-us".to_string()]);
}

#[test]
fn no_hits_yields_empty() {
    assert!(filter_contexts(&names(), "zzz").is_empty());
}

#[test]
fn advance_wraps_both_directions() {
    // len 5: 0 → +1 → 1; last (-1 from 0) wraps to 4.
    assert_eq!(advance_index(Some(0), 5, 1), Some(1));
    assert_eq!(advance_index(Some(0), 5, -1), Some(4));
    assert_eq!(advance_index(Some(4), 5, 1), Some(0));
}

#[test]
fn advance_clamps_stale_selection() {
    // Cursor 9 against a shrunken list of 2 clamps to 1, then applies delta.
    assert_eq!(advance_index(Some(9), 2, 1), Some(0));
    assert_eq!(advance_index(Some(9), 2, -1), Some(0));
}

#[test]
fn advance_empty_list_is_none() {
    assert_eq!(advance_index(Some(0), 0, 1), None);
    assert_eq!(advance_index(None, 0, -1), None);
}

#[test]
fn advance_none_selection_starts_at_zero() {
    assert_eq!(advance_index(None, 3, 1), Some(1));
    // Delta 0 on None lands on 0 (first row), not None.
    assert_eq!(advance_index(None, 3, 0), Some(0));
}
