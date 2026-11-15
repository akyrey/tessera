//! Bearer secrets (session cookies, invite links).
//!
//! The client holds a random 256-bit token; the database only stores its
//! SHA-256, so a leaked database cannot be replayed as sessions or invites.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use sha2::{Digest, Sha256};

const TOKEN_BYTES: usize = 32;
/// Length of a base64url-encoded 32-byte token without padding.
pub const ENCODED_LEN: usize = 43;

pub struct SecretToken {
    /// Sent to the client once; never stored.
    pub value: String,
    pub hash: Vec<u8>,
}

impl SecretToken {
    pub fn generate() -> Self {
        let mut bytes = [0u8; TOKEN_BYTES];
        rand::rng().fill_bytes(&mut bytes);
        let value = URL_SAFE_NO_PAD.encode(bytes);
        let hash = hash(&value);
        Self { value, hash }
    }

    /// Wraps a caller-supplied token (e.g. a configured bootstrap token).
    pub fn from_value(value: String) -> Self {
        let hash = hash(&value);
        Self { value, hash }
    }
}

pub fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

/// Minimum distinct byte values in an operator-supplied token. 32 random
/// bytes almost always have ~30; hand-typed or repeated patterns have few.
const MIN_DISTINCT_BYTES: usize = 16;

/// True when an operator-supplied token decodes to 32 bytes with enough variety
/// to have come from a CSPRNG (rejects e.g. `aaaa…` or a typed phrase).
pub fn is_strong(value: &str) -> bool {
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(value) else {
        return false;
    };
    let distinct = bytes.iter().collect::<std::collections::HashSet<_>>().len();
    bytes.len() == TOKEN_BYTES && distinct >= MIN_DISTINCT_BYTES
}

/// Cheap shape check before touching the database.
pub fn looks_valid(value: &str) -> bool {
    value.len() == ENCODED_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_unique_and_well_formed() {
        let a = SecretToken::generate();
        let b = SecretToken::generate();

        assert_ne!(a.value, b.value);
        assert!(looks_valid(&a.value), "{}", a.value);
        assert_eq!(a.hash, hash(&a.value));
        assert_eq!(a.hash.len(), 32);
    }

    #[test]
    fn generated_tokens_are_strong() {
        for _ in 0..100 {
            assert!(is_strong(&SecretToken::generate().value));
        }
    }

    #[test]
    fn weak_or_undecodable_tokens_are_not_strong() {
        assert!(!is_strong(&"a".repeat(ENCODED_LEN)));
        assert!(!is_strong(&"e2e-bootstrap-token".pad_to(ENCODED_LEN)));
        assert!(!is_strong("not base64!"));
        assert!(!is_strong(&URL_SAFE_NO_PAD.encode([7u8; 16])));
    }

    trait PadTo {
        fn pad_to(&self, len: usize) -> String;
    }

    impl PadTo for str {
        fn pad_to(&self, len: usize) -> String {
            format!("{self:x<len$}")
        }
    }

    #[test]
    fn rejects_malformed_tokens() {
        assert!(!looks_valid(""));
        assert!(!looks_valid("short"));
        assert!(!looks_valid(&"a".repeat(ENCODED_LEN + 1)));
        assert!(!looks_valid(&format!("{}=", "a".repeat(ENCODED_LEN - 1))));
    }
}
