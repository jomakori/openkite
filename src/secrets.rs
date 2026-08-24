//! Secret value redaction: masked by default, explicit per-item reveal.

/// The fixed placeholder shown in place of a masked secret value.
pub const MASKED_PLACEHOLDER: &str = "********";

/// Mask a secret value, returning a fixed placeholder that never contains the
/// plaintext (or any part of it).
pub fn mask(_value: &str) -> String {
    MASKED_PLACEHOLDER.to_string()
}

/// A secret value that is masked by default and can be explicitly revealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskedSecret {
    value: String,
    revealed: bool,
}

impl MaskedSecret {
    /// Wrap a plaintext value, masked by default.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            revealed: false,
        }
    }

    /// Reveal the plaintext.
    pub fn reveal(&mut self) {
        self.revealed = true;
    }

    /// Re-mask the value.
    pub fn hide(&mut self) {
        self.revealed = false;
    }

    /// The currently displayed value: placeholder when masked, plaintext when
    /// revealed.
    pub fn display(&self) -> &str {
        if self.revealed {
            &self.value
        } else {
            MASKED_PLACEHOLDER
        }
    }

    /// Whether the value is currently revealed.
    pub fn is_revealed(&self) -> bool {
        self.revealed
    }

    /// The raw plaintext. Callers should only surface this on explicit reveal.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Mask every value in a collection of key/value pairs, preserving the keys.
pub fn mask_all<'a, I>(pairs: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_string(), mask(value)))
        .collect()
}
