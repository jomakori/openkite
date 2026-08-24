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
