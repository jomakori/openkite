//! Integration tests for secret redaction.

use openkite::secrets::{mask, mask_all, MaskedSecret, MASKED_PLACEHOLDER};

#[test]
fn mask_never_leaks_plaintext() {
    let secret = "hunter2-password-123";
    let masked = mask(secret);
    assert_eq!(masked, MASKED_PLACEHOLDER);
    assert!(!masked.contains("hunter2"));
    assert!(!masked.contains("password"));
}

#[test]
fn masked_secret_hides_by_default() {
    let s = MaskedSecret::new("api-token-abc");
    assert!(!s.is_revealed());
    assert_eq!(s.display(), MASKED_PLACEHOLDER);
    assert_eq!(s.value(), "api-token-abc");
}

#[test]
fn reveal_then_hide_roundtrip() {
    let mut s = MaskedSecret::new("super-secret");
    assert_eq!(s.display(), MASKED_PLACEHOLDER);
    s.reveal();
    assert!(s.is_revealed());
    assert_eq!(s.display(), "super-secret");
    s.hide();
    assert!(!s.is_revealed());
    assert_eq!(s.display(), MASKED_PLACEHOLDER);
}

#[test]
fn mask_all_masks_values_keeps_keys() {
    let masked = mask_all([("password", "pw123"), ("token", "tok456")]);
    assert_eq!(
        masked[0],
        ("password".to_string(), MASKED_PLACEHOLDER.to_string())
    );
    assert_eq!(
        masked[1],
        ("token".to_string(), MASKED_PLACEHOLDER.to_string())
    );
}

#[test]
fn masked_secret_state_is_independent_per_instance() {
    let mut a = MaskedSecret::new("value-a");
    let mut b = MaskedSecret::new("value-b");
    a.reveal();
    assert!(a.is_revealed());
    assert!(!b.is_revealed());
    assert_eq!(a.display(), "value-a");
    assert_eq!(b.display(), MASKED_PLACEHOLDER);
}

#[test]
fn masked_secret_value_still_holds_plaintext_when_masked() {
    let s = MaskedSecret::new("plaintext-stays");
    assert!(!s.is_revealed());
    assert_eq!(s.display(), MASKED_PLACEHOLDER);
    // The `MaskedSecret` is the trust boundary, not the storage: the
    // plaintext is reachable only through the explicit `value()` API.
    assert_eq!(s.value(), "plaintext-stays");
}

#[test]
fn map_reveal_one_key_leaves_others_masked() {
    let mut map = std::collections::HashMap::new();
    map.insert("a".to_string(), MaskedSecret::new("val-a"));
    map.insert("b".to_string(), MaskedSecret::new("val-b"));
    map.get_mut("a").unwrap().reveal();
    assert_eq!(map["a"].display(), "val-a");
    assert_eq!(map["b"].display(), MASKED_PLACEHOLDER);
}

#[test]
fn map_bulk_reveal_calls_reveal_on_every_entry() {
    let mut map = std::collections::HashMap::new();
    map.insert("a".to_string(), MaskedSecret::new("val-a"));
    map.insert("b".to_string(), MaskedSecret::new("val-b"));
    for v in map.values_mut() {
        v.reveal();
    }
    assert_eq!(map["a"].display(), "val-a");
    assert_eq!(map["b"].display(), "val-b");
}

#[test]
fn bulk_reveal_predicate_matches_exact_name_only() {
    use openkite::components::secret_detail::bulk_reveal_predicate;
    assert!(bulk_reveal_predicate("foo", "foo"));
    assert!(!bulk_reveal_predicate("foo", "Foo"));
    assert!(bulk_reveal_predicate(" foo ", "foo"));
    assert!(!bulk_reveal_predicate("foo bar", "foo"));
}
