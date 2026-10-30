//! HMAC-SHA256 through the selected native provider.
//!
//! `rustls::crypto::tls13::Hkdf::hmac_sign` is explicitly RFC 2104 HMAC
//! with the supplied key, not HKDF expansion. SHA-256 keys longer than its
//! 64-byte block size are hashed once as required by RFC 2104 §2.

use rustls::crypto::tls13::OkmBlock;
use zeroize::Zeroizing;

use super::{CryptoError, backend, constant_time_eq, hash::Sha256};

/// Portable message-size limit imposed by the CNG HMAC adapter.
pub const MAX_MESSAGE_LEN: usize = u32::MAX as usize;

/// An owned HMAC-SHA256 key, redacted in debug output and zeroized on drop.
/// Facade-owned normalization buffers are zeroized; backend/intermediate
/// copies made by rustls or OS crypto APIs are outside that guarantee.
pub struct HmacSha256 {
    key: OkmBlock,
}

impl std::fmt::Debug for HmacSha256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HmacSha256").finish_non_exhaustive()
    }
}

impl HmacSha256 {
    /// Construct a key without silently truncating it.
    ///
    /// # Errors
    /// FIPS builds reject keys below 14 bytes (SP 800-131A Rev. 2 §10).
    ///
    /// # Panics
    /// Panics if normalizing a long key requires an unavailable backend.
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        if cfg!(feature = "fips") && key.len() < 14 {
            return Err(CryptoError::HmacKeyTooShort);
        }
        let key = if key.len() > 64 {
            let normalized = Zeroizing::new(Sha256::digest(key));
            OkmBlock::new(normalized.as_ref())
        } else {
            OkmBlock::new(key)
        };
        Ok(Self { key })
    }

    /// Sign bytes, returning the full 32-byte tag.
    ///
    /// # Errors
    /// Returns `MessageTooLong` above [`MAX_MESSAGE_LEN`] bytes, or a backend
    /// configuration error. Native operational failures may panic inside rustls.
    pub fn sign(&self, message: &[u8]) -> Result<[u8; 32], CryptoError> {
        validate_message_len(message.len())?;
        let tag = backend::sha256_suite()?
            .hkdf_provider
            .hmac_sign(&self.key, message);
        let mut bytes = [0; 32];
        bytes.copy_from_slice(tag.as_ref());
        Ok(bytes)
    }

    /// Verify a full-length tag without leaking the mismatch position.
    ///
    /// # Errors
    /// Returns `InvalidTag` on mismatch, or the same errors as [`Self::sign`].
    pub fn verify(&self, message: &[u8], tag: &[u8]) -> Result<(), CryptoError> {
        if constant_time_eq(&self.sign(message)?, tag) {
            Ok(())
        } else {
            Err(CryptoError::InvalidTag)
        }
    }
}

fn validate_message_len(len: usize) -> Result<(), CryptoError> {
    if len > MAX_MESSAGE_LEN {
        Err(CryptoError::MessageTooLong)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "mac_tests.rs"]
mod tests;
