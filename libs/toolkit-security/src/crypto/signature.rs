//! Signatures through the selected provider, plus ordinary-build Ed25519 compatibility.
//!
//! The current native FIPS providers do not supply Ed25519. Every operation
//! rejects it under `fips`; it never falls back to ordinary AWS-LC.

use aws_lc_rs::signature::{self, KeyPair, UnparsedPublicKey};

use super::CryptoError;

pub use super::rsa::{RsaAlgorithm, verify_rsa_components};

/// An owned Ed25519 signing key. Private backend types are not re-exported.
pub struct Ed25519KeyPair {
    inner: signature::Ed25519KeyPair,
}

impl Ed25519KeyPair {
    /// Generate a key using the ordinary-build AWS-LC CSPRNG.
    ///
    /// # Errors
    /// Returns `UnsupportedAlgorithm` under FIPS or `InvalidKey` on failure.
    pub fn generate() -> Result<Self, CryptoError> {
        ensure_supported()?;
        signature::Ed25519KeyPair::generate()
            .map(|inner| Self { inner })
            .map_err(|_| CryptoError::InvalidKey)
    }

    /// Import the exact 32-byte seed used by existing signed-URL deployments.
    ///
    /// # Errors
    /// Returns `UnsupportedAlgorithm` under FIPS, or `InvalidKey` for invalid seeds.
    pub fn from_seed(seed: &[u8]) -> Result<Self, CryptoError> {
        ensure_supported()?;
        if seed.len() != 32 {
            return Err(CryptoError::InvalidKey);
        }
        signature::Ed25519KeyPair::from_seed_unchecked(seed)
            .map(|inner| Self { inner })
            .map_err(|_| CryptoError::InvalidKey)
    }

    /// Return the 32-byte public key, preserving the existing raw key format.
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        self.inner.public_key().as_ref()
    }

    /// Sign the message, preserving the existing 64-byte signature format.
    ///
    /// # Errors
    /// Returns `UnsupportedAlgorithm` under FIPS or `InvalidSignature` on failure.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, CryptoError> {
        ensure_supported()?;
        self.inner
            .try_sign(message)
            .map(|signature| signature.as_ref().to_vec())
            .map_err(|_| CryptoError::InvalidSignature)
    }
}

/// Verify an existing raw Ed25519 signature and public key.
///
/// # Errors
/// Returns `UnsupportedAlgorithm` under FIPS, or `InvalidSignature` on mismatch
/// or malformed key/signature bytes.
pub fn verify_ed25519(key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), CryptoError> {
    ensure_supported()?;
    UnparsedPublicKey::new(&signature::ED25519, key)
        .verify(message, signature)
        .map_err(|_| CryptoError::InvalidSignature)
}

fn ensure_supported() -> Result<(), CryptoError> {
    if cfg!(feature = "fips") {
        Err(CryptoError::UnsupportedAlgorithm)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "signature_tests.rs"]
mod tests;
