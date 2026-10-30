//! JWT signature verification through the facade, with jsonwebtoken's existing
//! claim validation. This adapter owns the signature-disabled second pass;
//! callers cannot obtain claims without first verifying the original token.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::errors::{ErrorKind, Result};
use jsonwebtoken::{
    Algorithm, AlgorithmFamily, DecodingKey, DecodingKeyKind, Header, TokenData, Validation,
};
use pkcs1::der::{Decode, Encode};
use serde::de::DeserializeOwned;

use super::{
    backend,
    signature::{RsaAlgorithm, verify_rsa_components},
};

/// Decode a JWT, verifying its signature with the configured OS backend and
/// validating claims with the caller's original jsonwebtoken policy.
///
/// Signature verification is always required, even if the caller disabled it
/// in `validation`. Existing RS/PS and ES256/384 consumers are supported;
/// other algorithms fail without fallback.
///
/// # Errors
/// Returns jsonwebtoken errors for invalid signatures, keys, algorithms, token
/// encodings or claims. The original token bytes are verified before decoding.
pub fn decode<T: DeserializeOwned>(
    token: &str,
    key: &DecodingKey,
    validation: &Validation,
) -> Result<TokenData<T>> {
    let header = jsonwebtoken::decode_header(token)?;
    if validation.algorithms.is_empty() {
        return Err(ErrorKind::MissingAlgorithm.into());
    }
    if !validation.algorithms.contains(&header.alg)
        || validation
            .algorithms
            .iter()
            .any(|alg| family(*alg) != key.family())
    {
        return Err(ErrorKind::InvalidAlgorithm.into());
    }
    verify(token, key, &header)?;
    let mut claims_policy = validation.clone();
    // dangerous::insecure_decode would also skip claims validation.
    #[allow(deprecated)]
    claims_policy.insecure_disable_signature_validation();
    // jsonwebtoken 10.3 still constructs a verifier here, but its AWS-LC
    // constructors only check the family and clone the key. No primitive is
    // called: signature decoding and verify() are behind validate_signature.
    jsonwebtoken::decode(token, key, &claims_policy)
}

// Private: header must come from decode_header(token) in the verified adapter.
fn verify(token: &str, key: &DecodingKey, header: &Header) -> Result<()> {
    let parts: Vec<_> = token.splitn(4, '.').collect();
    if parts.len() != 3 {
        return Err(ErrorKind::InvalidToken.into());
    }
    if family(header.alg) != key.family() {
        return Err(ErrorKind::InvalidAlgorithm.into());
    }
    let message = &token.as_bytes()[..parts[0].len() + 1 + parts[1].len()];
    let signature = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| jsonwebtoken::errors::Error::from(ErrorKind::InvalidSignature))?;
    let invalid = || jsonwebtoken::errors::Error::from(ErrorKind::InvalidSignature);
    match header.alg {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::PS256
        | Algorithm::PS384
        | Algorithm::PS512 => {
            let algorithm = match header.alg {
                Algorithm::RS256 => RsaAlgorithm::Pkcs1Sha256,
                Algorithm::RS384 => RsaAlgorithm::Pkcs1Sha384,
                Algorithm::RS512 => RsaAlgorithm::Pkcs1Sha512,
                Algorithm::PS256 => RsaAlgorithm::PssSha256,
                Algorithm::PS384 => RsaAlgorithm::PssSha384,
                Algorithm::PS512 => RsaAlgorithm::PssSha512,
                _ => unreachable!("RSA branch"),
            };
            match key.kind() {
                DecodingKeyKind::RsaModulusExponent { n, e } => {
                    verify_rsa_components(algorithm, n, e, message, &signature)
                        .map_err(|_| invalid())
                }
                DecodingKeyKind::SecretOrDer(der) => {
                    let key = rsa_public_key(der).map_err(|_| invalid())?;
                    verify_rsa_components(
                        algorithm,
                        key.modulus.as_bytes(),
                        key.public_exponent.as_bytes(),
                        message,
                        &signature,
                    )
                    .map_err(|_| invalid())
                }
            }
        }
        Algorithm::ES256 | Algorithm::ES384 => {
            verify_ecdsa(header.alg, key.as_bytes(), message, &signature).map_err(|_| invalid())
        }
        // HMAC verifier constructors initialize AWS-LC state even with
        // verification disabled. This adapter supports only the existing
        // RSA/ECDSA consumers whose constructors perform no second-pass crypto.
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 | Algorithm::EdDSA => {
            Err(ErrorKind::InvalidAlgorithm.into())
        }
    }
}

fn rsa_public_key(
    bytes: &[u8],
) -> std::result::Result<pkcs1::RsaPublicKey<'_>, super::CryptoError> {
    if let Ok(key) = pkcs1::RsaPublicKey::from_der(bytes) {
        return Ok(key);
    }
    let spki = spki::SubjectPublicKeyInfoRef::from_der(bytes)
        .map_err(|_| super::CryptoError::InvalidKey)?;
    // Only rsaEncryption: never discard RSA-PSS algorithm/key constraints.
    if spki.algorithm.oid != pkcs1::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1")
        || spki
            .algorithm
            .parameters
            .is_some_and(|params| params != pkcs1::der::asn1::AnyRef::NULL)
    {
        return Err(super::CryptoError::InvalidKey);
    }
    let bytes = spki
        .subject_public_key
        .as_bytes()
        .ok_or(super::CryptoError::InvalidKey)?;
    pkcs1::RsaPublicKey::from_der(bytes).map_err(|_| super::CryptoError::InvalidKey)
}

fn verify_ecdsa(
    algorithm: Algorithm,
    key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> std::result::Result<(), super::CryptoError> {
    use rustls::pki_types::alg_id;
    let (size, scheme, key_id, signature_id) = match algorithm {
        Algorithm::ES256 => (
            32,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            alg_id::ECDSA_P256,
            alg_id::ECDSA_SHA256,
        ),
        Algorithm::ES384 => (
            48,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            alg_id::ECDSA_P384,
            alg_id::ECDSA_SHA384,
        ),
        _ => return Err(super::CryptoError::UnsupportedAlgorithm),
    };
    if signature.len() != size * 2 || key.len() != size * 2 + 1 || key[0] != 4 {
        return Err(super::CryptoError::InvalidSignature);
    }
    if message.len() > u32::MAX as usize {
        return Err(super::CryptoError::MessageTooLong);
    }
    // JOSE uses fixed-width r||s; the TLS verifier expects an ASN.1 sequence
    // of positive INTEGERs. Each encoded scalar and the sequence are <128B.
    let mut der = vec![0x30, 0];
    for scalar in signature.chunks_exact(size) {
        let integer =
            pkcs1::UintRef::new(scalar).map_err(|_| super::CryptoError::InvalidSignature)?;
        der.extend(
            integer
                .to_der()
                .map_err(|_| super::CryptoError::InvalidSignature)?,
        );
    }
    der[1] = u8::try_from(der.len() - 2).map_err(|_| super::CryptoError::InvalidSignature)?;
    let algorithms = backend::provider()?
        .signature_verification_algorithms
        .mapping
        .iter()
        .find(|(candidate, _)| *candidate == scheme)
        .map(|(_, algorithms)| *algorithms)
        .ok_or(super::CryptoError::UnsupportedAlgorithm)?;
    if algorithms.iter().any(|verifier| {
        verifier.public_key_alg_id() == key_id
            && verifier.signature_alg_id() == signature_id
            && (!cfg!(feature = "fips") || verifier.fips())
            && verifier.verify_signature(key, message, &der).is_ok()
    }) {
        Ok(())
    } else {
        Err(super::CryptoError::InvalidSignature)
    }
}

fn family(algorithm: Algorithm) -> AlgorithmFamily {
    match algorithm {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::PS256
        | Algorithm::PS384
        | Algorithm::PS512 => AlgorithmFamily::Rsa,
        Algorithm::ES256 | Algorithm::ES384 => AlgorithmFamily::Ec,
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => AlgorithmFamily::Hmac,
        Algorithm::EdDSA => AlgorithmFamily::Ed,
    }
}

#[cfg(test)]
#[path = "jwt_tests.rs"]
mod tests;
