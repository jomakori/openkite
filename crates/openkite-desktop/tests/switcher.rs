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
    let got = filter_contexts(&names(), "prod");
    assert_eq!(got, vec!["prod".to_string(), "prod-us".to_string()]);
}

#[test]
fn no_hits_yields_empty() {
    assert!(filter_contexts(&names(), "zzz").is_empty());
}

#[test]
fn advance_wraps_both_directions() {
    assert_eq!(advance_index(Some(0), 5, 1), Some(1));
    assert_eq!(advance_index(Some(0), 5, -1), Some(4));
    assert_eq!(advance_index(Some(4), 5, 1), Some(0));
}

#[test]
fn advance_clamps_stale_selection() {
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
    assert_eq!(advance_index(None, 3, 0), Some(0));
}
