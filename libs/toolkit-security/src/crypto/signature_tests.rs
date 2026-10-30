use super::{CryptoError, Ed25519KeyPair, verify_ed25519};

#[test]
fn ed25519_follows_the_selected_policy() {
    if cfg!(feature = "fips") {
        assert!(matches!(
            Ed25519KeyPair::generate(),
            Err(CryptoError::UnsupportedAlgorithm)
        ));
        assert!(matches!(
            Ed25519KeyPair::from_seed(&[0; 32]),
            Err(CryptoError::UnsupportedAlgorithm)
        ));
        assert_eq!(
            verify_ed25519(&[0; 32], b"", &[0; 64]),
            Err(CryptoError::UnsupportedAlgorithm)
        );
        return;
    }

    let key = Ed25519KeyPair::generate().unwrap();
    let signature = key.sign(b"message").unwrap();
    verify_ed25519(key.public_key(), b"message", &signature).unwrap();
    assert_eq!(
        verify_ed25519(key.public_key(), b"changed", &signature),
        Err(CryptoError::InvalidSignature)
    );
    assert_eq!(
        verify_ed25519(&[0; 31], b"message", &signature),
        Err(CryptoError::InvalidSignature)
    );
    assert_eq!(
        verify_ed25519(key.public_key(), b"message", &signature[..63]),
        Err(CryptoError::InvalidSignature)
    );
}
