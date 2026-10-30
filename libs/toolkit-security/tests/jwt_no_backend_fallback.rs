#![cfg(feature = "jwt")]

use aws_lc_rs::signature::KeyPair;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::crypto::{CryptoProvider, JwkUtils, JwtVerifier};
use jsonwebtoken::signature::Verifier;
use jsonwebtoken::{Algorithm, DecodingKey, Header, Validation};
use std::sync::atomic::{AtomicUsize, Ordering};

static CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);
struct ForbiddenVerifier;

impl Verifier<Vec<u8>> for ForbiddenVerifier {
    fn verify(&self, _: &[u8], _: &Vec<u8>) -> Result<(), jsonwebtoken::signature::Error> {
        panic!("claim-validation pass must never execute a second crypto backend")
    }
}

impl JwtVerifier for ForbiddenVerifier {
    fn algorithm(&self) -> Algorithm {
        Algorithm::RS256
    }
}

static POISON: CryptoProvider = CryptoProvider {
    signer_factory: |_, _| panic!("no signing in this test"),
    verifier_factory: |_, _| {
        CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(ForbiddenVerifier))
    },
    jwk_utils: JwkUtils::new_unimplemented(),
};

#[test]
fn claims_pass_does_not_execute_jsonwebtoken_crypto() {
    POISON.install_default().unwrap();
    let encoding = jsonwebtoken::EncodingKey::from_rsa_pem(include_bytes!(
        "../src/crypto/testdata/rsa-private.pem"
    ))
    .unwrap();
    let aws_key = aws_lc_rs::signature::RsaKeyPair::from_pkcs8(encoding.inner())
        .or_else(|_| aws_lc_rs::signature::RsaKeyPair::from_der(encoding.inner()))
        .unwrap();
    let key = DecodingKey::from_rsa_der(aws_key.public_key().as_ref());
    let header =
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&Header::new(Algorithm::RS256)).unwrap());
    let payload = URL_SAFE_NO_PAD.encode(br#"{"sub":"alice","exp":4000000000}"#);
    let message = format!("{header}.{payload}");
    // Independent signing oracle. No jsonwebtoken global provider is used.
    let signer = (jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER.signer_factory)(
        &Algorithm::RS256,
        &encoding,
    )
    .unwrap();
    let signature = signer.try_sign(message.as_bytes()).unwrap();
    let token = format!("{message}.{}", URL_SAFE_NO_PAD.encode(signature));
    let decoded = toolkit_security::crypto::jwt::decode::<serde_json::Value>(
        &token,
        &key,
        &Validation::new(Algorithm::RS256),
    )
    .unwrap();
    assert_eq!(decoded.claims["sub"], "alice");
    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 1);
    let invalid = format!("{message}.{}", URL_SAFE_NO_PAD.encode([0; 256]));
    assert!(
        toolkit_security::crypto::jwt::decode::<serde_json::Value>(
            &invalid,
            &key,
            &Validation::new(Algorithm::RS256)
        )
        .is_err()
    );
    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 1);
}
