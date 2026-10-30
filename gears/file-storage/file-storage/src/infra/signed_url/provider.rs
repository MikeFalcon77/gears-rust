//! Pluggable signature provider (ADR-0004 "FIPS posture").
//!
//! The token codec MUST call this abstraction and never a crypto crate directly, so the
//! signing primitive (and, in a FIPS deployment, the validated module behind it) is
//! replaceable without touching the codec.
//!
//! `Ed25519Provider` uses the toolkit-security facade. Its existing key and
//! token formats remain unchanged; the facade explicitly refuses Ed25519 in
//! FIPS builds because the configured native providers do not provide it.

use std::sync::Arc;

use toolkit_security::crypto::signature::{Ed25519KeyPair, verify_ed25519};

use crate::domain::error::DomainError;

/// Minting side (control plane): signs payloads and yields a matching verifier.
pub trait SignatureProvider: Send + Sync {
    /// Sign `message`, returning the raw signature bytes.
    fn sign(&self, message: &[u8]) -> Vec<u8>;
    /// The public-key bytes a peer verifier needs (shared with the sidecar).
    fn public_key(&self) -> Vec<u8>;
    /// A verifier for the tokens this provider mints.
    fn verifier(&self) -> Arc<dyn SignatureVerifier>;
}

/// Verifying side (sidecar): checks a signature against the public key. Holds no
/// secret and can never mint a token.
pub trait SignatureVerifier: Send + Sync {
    /// Verify `signature` over `message`; `Err` on any mismatch.
    fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), DomainError>;
}

/// Default provider: Ed25519 through the toolkit-security crypto facade.
pub struct Ed25519Provider {
    key_pair: Ed25519KeyPair,
    public_key: Vec<u8>,
}

impl Ed25519Provider {
    /// Generate a fresh static keypair (no rotation).
    pub fn generate() -> Result<Self, DomainError> {
        let key_pair = Ed25519KeyPair::generate().map_err(|error| {
            DomainError::token_invalid(format!("failed to generate signing key: {error}"))
        })?;
        let public_key = key_pair.public_key().to_vec();
        Ok(Self {
            key_pair,
            public_key,
        })
    }

    /// Construct from a 32-byte Ed25519 seed, so the keypair is stable across restarts.
    pub fn from_seed(seed: &[u8]) -> Result<Self, DomainError> {
        let key_pair = Ed25519KeyPair::from_seed(seed).map_err(|error| {
            DomainError::token_invalid(format!(
                "invalid Ed25519 signing seed (expected 32 bytes): {error}"
            ))
        })?;
        let public_key = key_pair.public_key().to_vec();
        Ok(Self {
            key_pair,
            public_key,
        })
    }
}

impl SignatureProvider for Ed25519Provider {
    fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.key_pair
            .sign(message)
            .unwrap_or_else(|error| panic!("signed-URL signing failed: {error}"))
    }

    fn public_key(&self) -> Vec<u8> {
        self.public_key.clone()
    }

    fn verifier(&self) -> Arc<dyn SignatureVerifier> {
        Arc::new(Ed25519Verifier::new(self.public_key.clone()))
    }
}

/// Verifier for `Ed25519Provider`-minted tokens (public key only).
pub struct Ed25519Verifier {
    public_key: Vec<u8>,
}

impl Ed25519Verifier {
    /// Construct from raw Ed25519 public-key bytes (e.g. sidecar config).
    #[must_use]
    pub fn new(public_key: Vec<u8>) -> Self {
        Self { public_key }
    }
}

impl SignatureVerifier for Ed25519Verifier {
    fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), DomainError> {
        verify_ed25519(&self.public_key, message, signature).map_err(|error| {
            DomainError::token_invalid(format!("signature verification failed: {error}"))
        })
    }
}
