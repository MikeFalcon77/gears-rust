use super::*;

#[test]
fn rejects_noncanonical_or_weak_public_components() {
    let valid = vec![0xff; 256];
    assert_eq!(validate_components(&valid, &[1, 0, 1]), Ok(()));
    for n in [vec![], vec![0; 256], vec![0xff; 255], vec![0xff; 1025]] {
        assert_eq!(validate_components(&n, &[3]), Err(CryptoError::InvalidKey));
    }
    let mut too_small = valid.clone();
    too_small[0] = 0x7f;
    assert_eq!(
        validate_components(&too_small, &[3]),
        Err(CryptoError::InvalidKey)
    );
    let mut even = valid.clone();
    even[255] = 0xfe;
    assert_eq!(
        validate_components(&even, &[3]),
        Err(CryptoError::InvalidKey)
    );
    for e in [
        vec![],
        vec![0, 3],
        vec![1],
        vec![2],
        vec![4],
        vec![2, 0, 0, 0, 1],
    ] {
        assert_eq!(
            validate_components(&valid, &e),
            Err(CryptoError::InvalidKey)
        );
    }
    assert_eq!(
        validate_components(&vec![0xff; 1024], &[1, 0xff, 0xff, 0xff, 0xff]),
        Ok(())
    );
}

#[test]
fn rejects_wrong_signature_size_before_dispatch() {
    assert_eq!(
        verify_rsa_components(
            RsaAlgorithm::Pkcs1Sha256,
            &vec![0xff; 256],
            &[3],
            b"message",
            &[0; 255]
        ),
        Err(CryptoError::InvalidSignature)
    );
}
