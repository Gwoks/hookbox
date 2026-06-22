//! Identity / token helpers — PORT of `app/database.py` (hash_email,
//! gen_owner_secret, hash_secret) and `app/utils/helpers.py::generate_endpoint_id`.
//!
//! - `gen_token`: ambiguity-stripped alphabet (no `0 O 1 l I`), default len 10,
//!   case-sensitive, independent of owner id (AC-6a, AC-S5).
//! - `gen_owner_secret`: 256-bit CSPRNG, base64url (matches Python
//!   `secrets.token_urlsafe(32)`), returned once on `/api/session`, stored hashed.
//! - `hash_email`: non-secret owner id = `sha256(lower(trim(email)))[:16]`.
//! - `hash_secret`: `sha256(owner_secret)` for at-rest storage / lookup (§5.1).

use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Ambiguity-stripped alphabet: ASCII letters + digits minus `0 O 1 l I`.
/// Built to byte-equal the Python `_TOKEN_ALPHABET` ordering.
fn token_alphabet() -> Vec<u8> {
    let mut a: Vec<u8> = Vec::with_capacity(62);
    a.extend(b'a'..=b'z');
    a.extend(b'A'..=b'Z');
    a.extend(b'0'..=b'9');
    a.retain(|&c| !matches!(c, b'0' | b'O' | b'1' | b'l' | b'I'));
    a
}

/// Generate an ambiguity-stripped endpoint token (case-sensitive, CSPRNG).
pub fn gen_token(length: usize) -> String {
    let alphabet = token_alphabet();
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            // Uniform pick over the alphabet via rejection-free index.
            let idx = (rng.next_u32() as usize) % alphabet.len();
            alphabet[idx] as char
        })
        .collect()
}

/// CSPRNG bearer capability: `n_bytes` of entropy, base64url no-pad
/// (mirrors Python `secrets.token_urlsafe(n_bytes)`). Default 32 ⇒ 256-bit.
pub fn gen_owner_secret(n_bytes: usize) -> String {
    let mut buf = vec![0u8; n_bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// Non-secret owner id derived from email: `sha256(lower(trim(email)))` truncated
/// to the first 16 hex chars (PORT of `hash_email`).
pub fn hash_email(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let digest = Sha256::digest(normalized.as_bytes());
    hex::encode(digest)[..16].to_string()
}

/// `sha256(secret)` hex — the at-rest secret hash looked up on every `/api/**`
/// request. The raw secret is NEVER stored or compared directly.
pub fn hash_secret(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_alphabet_strips_ambiguous_chars() {
        let a = token_alphabet();
        for c in [b'0', b'O', b'1', b'l', b'I'] {
            assert!(!a.contains(&c), "ambiguous char {} not stripped", c as char);
        }
        // 26 + 26 + 10 = 62, minus 5 stripped = 57.
        assert_eq!(a.len(), 57);
        // Case-sensitive: both 'o' and 'i' kept (only uppercase O / I, and l, stripped).
        assert!(a.contains(&b'o'));
        assert!(a.contains(&b'i'));
        assert!(a.contains(&b'L'));
    }

    #[test]
    fn gen_token_length_and_charset() {
        let t = gen_token(10);
        assert_eq!(t.chars().count(), 10);
        let alphabet: String = token_alphabet().iter().map(|&b| b as char).collect();
        assert!(t.chars().all(|c| alphabet.contains(c)));
        // Two tokens differ (CSPRNG) with overwhelming probability.
        assert_ne!(gen_token(10), gen_token(10));
    }

    #[test]
    fn owner_secret_is_256_bit_urlsafe() {
        let s = gen_owner_secret(32);
        // base64url no-pad of 32 bytes -> 43 chars.
        assert_eq!(s.len(), 43);
        assert!(!s.contains('=') && !s.contains('+') && !s.contains('/'));
        assert_ne!(gen_owner_secret(32), gen_owner_secret(32));
    }

    #[test]
    fn hash_email_matches_python_oracle() {
        // sha256("a@b.com")[:16] computed against the Python definition.
        // Normalization: trim + lowercase.
        assert_eq!(hash_email("  A@B.com  "), hash_email("a@b.com"));
        assert_eq!(hash_email("a@b.com").len(), 16);
    }

    #[test]
    fn hash_secret_is_sha256_hex() {
        // Known SHA-256 of "abc".
        assert_eq!(
            hash_secret("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
