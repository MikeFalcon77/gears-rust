//! Value-fingerprint fence — the crypto and constants binding a secret's
//! backend value to the metadata row that governs its visibility.
//!
//! Every API write stamps `credstore_secrets.value_fp` with
//! `HMAC-SHA256(fence_key, value)`; every read recomputes it from the value
//! the backend returned and serves the value only when the row agrees. The
//! gear performs a dual write (backend value + DB metadata) with no
//! transaction spanning both stores, so two concurrent unconditional PUTs
//! can interleave crosswise; the fingerprint makes the poisoned combination
//! unreadable (fail-closed 404) instead of a cross-tenant disclosure. See
//! `docs/features/001-value-fingerprint-fence.md`.
//!
//! The fence key is deployment state, not configuration: auto-generated and
//! stored in the value-store backend itself under [`FENCE_KEY_REF`] (nil
//! tenant, no metadata row — unreachable through the API by construction).
//! Split knowledge: fingerprints live in the gear DB, the key lives with
//! the values, so a read-only DB compromise alone cannot dictionary-test
//! fingerprints, and backend compromise yields the plaintexts anyway.
//! The fingerprint itself never leaves the gear (no API field, header,
//! or log line).

use toolkit_security::crypto::{mac::HmacSha256, rng};

use crate::domain::error::DomainError;

/// Reserved backend reference the auto-generated fence key is stored under
/// (tenant = nil UUID, owner = `None`). It has no `credstore_secrets` row,
/// so no API path can resolve, overwrite, or delete it: resolution always
/// starts from a metadata row, and external callers always carry a real
/// tenant, never nil.
pub const FENCE_KEY_REF: &str = "cfs-internal-fence-key";

/// Fence-key id stamped into `fp_key_id` alongside every fingerprint.
/// v1 uses a single key; the id column plus the reserved-reference naming
/// are the keyring groundwork for rotation (out of scope — see the feature
/// spec §3, `algo-fp-compute-verify`).
pub const CURRENT_FENCE_KEY_ID: i16 = 1;

/// Fence-key length in bytes.
pub const FENCE_KEY_LEN: usize = 32;

/// Max jitter (ms) before the fence-key bootstrap read/generate. De-synchronises
/// replicas that cold-start together so a create collision is unlikely. The
/// plugin port has no atomic create-if-absent, so this is a probabilistic
/// defence, not mutual exclusion; residual races stay fail-closed (mismatch →
/// 404 → self-heal on rewrite). One-time cost on a replica's first fence op.
/// Zero under unit tests: the jitter is a timing-only knob, not logic under
/// test, and real sleeps would make the suite slow and time-sensitive. (Lives
/// here, not in the service module, so its test override does not put a
/// `#[cfg(test)]` item in a file that has a companion `_tests.rs` — DE1101.)
#[cfg(not(test))]
pub const BOOTSTRAP_JITTER_MAX_MS: u64 = 500;
#[cfg(test)]
pub const BOOTSTRAP_JITTER_MAX_MS: u64 = 0;

/// `HMAC-SHA256(fence_key, value)` — the full 32-byte fingerprint stored in
/// `credstore_secrets.value_fp`. Never truncated: the value is internal-only,
/// and truncation would only add fence-collision surface.
///
/// The toolkit-security facade selects AWS-LC, corecrypto or CNG according
/// to the build's platform/FIPS policy.
///
/// # Panics
/// Panics for an invalid key or a value beyond the facade's portable MAC limit.
#[must_use]
pub fn compute_fp(key: &[u8], value: &[u8]) -> Vec<u8> {
    let key = HmacSha256::new(key).unwrap_or_else(|error| panic!("invalid fence key: {error}"));
    key.sign(value)
        .unwrap_or_else(|error| panic!("fence fingerprint failed: {error}"))
        .to_vec()
}

/// Constant-time fingerprint verification through the crypto facade.
#[must_use]
pub fn verify_fp(key: &[u8], value: &[u8], fp: &[u8]) -> bool {
    HmacSha256::new(key)
        .and_then(|key| key.verify(value, fp))
        .is_ok()
}

/// Generate a fresh [`FENCE_KEY_LEN`]-byte fence key from the platform CSPRNG
/// selected by the facade's platform/FIPS policy.
///
/// # Errors
///
/// Returns [`DomainError::Internal`] if the system RNG fails.
pub fn generate_key() -> Result<Vec<u8>, DomainError> {
    let mut buf = [0u8; FENCE_KEY_LEN];
    rng::fill(&mut buf)
        .map_err(|_| DomainError::internal("fence: system RNG failed to generate a fence key"))?;
    Ok(buf.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fp_is_deterministic_and_key_and_value_sensitive() {
        let k1 = vec![1u8; FENCE_KEY_LEN];
        let k2 = vec![2u8; FENCE_KEY_LEN];
        let a = compute_fp(&k1, b"value");
        assert_eq!(a, compute_fp(&k1, b"value"));
        assert_eq!(a.len(), 32);
        assert_ne!(a, compute_fp(&k2, b"value"));
        assert_ne!(a, compute_fp(&k1, b"other"));
    }

    #[test]
    fn verify_accepts_matching_and_rejects_foreign_fp() {
        let key = generate_key().expect("key");
        let fp = compute_fp(&key, b"secret");
        assert!(verify_fp(&key, b"secret", &fp));
        assert!(!verify_fp(&key, b"tampered", &fp));
        assert!(!verify_fp(&generate_key().expect("key"), b"secret", &fp));
        // Truncated/foreign-length fingerprints must not verify.
        assert!(!verify_fp(&key, b"secret", &fp[..8]));
    }

    #[test]
    fn generated_keys_are_distinct_and_sized() {
        let a = generate_key().expect("key");
        let b = generate_key().expect("key");
        assert_eq!(a.len(), FENCE_KEY_LEN);
        assert_ne!(a, b);
    }

    #[test]
    fn fence_key_ref_is_a_valid_secret_ref() {
        assert!(credstore_sdk::SecretRef::new(FENCE_KEY_REF).is_ok());
    }
}
