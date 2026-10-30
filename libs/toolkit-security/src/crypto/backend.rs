use std::sync::OnceLock;

use rustls::Tls13CipherSuite;
use rustls::crypto::CryptoProvider;
use rustls::crypto::hash::HashAlgorithm;

use super::CryptoError;

static PROVIDER: OnceLock<Result<CryptoProvider, CryptoError>> = OnceLock::new();

// An unrelated library installing a TLS provider must not change application crypto.
pub(super) fn configured_provider() -> CryptoProvider {
    #[cfg(all(feature = "fips", target_os = "macos"))]
    {
        rustls_corecrypto_provider::fips_provider()
    }
    #[cfg(all(feature = "fips", target_os = "windows"))]
    {
        rustls_cng_crypto::fips_provider()
    }
    #[cfg(not(all(feature = "fips", any(target_os = "macos", target_os = "windows"))))]
    {
        rustls::crypto::aws_lc_rs::default_provider()
    }
}

pub(super) fn provider() -> Result<&'static CryptoProvider, CryptoError> {
    PROVIDER
        .get_or_init(|| {
            let provider = configured_provider();
            validate_provider(&provider, cfg!(feature = "fips"))?;
            Ok(provider)
        })
        .as_ref()
        .map_err(|error| *error)
}

pub(super) fn sha256_suite() -> Result<&'static Tls13CipherSuite, CryptoError> {
    find_sha256(provider()?)
}

fn validate_provider(provider: &CryptoProvider, require_fips: bool) -> Result<(), CryptoError> {
    if require_fips && !provider.fips() {
        Err(CryptoError::FipsUnavailable)
    } else {
        Ok(())
    }
}

pub(super) fn find_sha256(
    provider: &CryptoProvider,
) -> Result<&'static Tls13CipherSuite, CryptoError> {
    provider
        .cipher_suites
        .iter()
        .filter_map(rustls::SupportedCipherSuite::tls13)
        .find(|suite| suite.common.hash_provider.algorithm() == HashAlgorithm::SHA256)
        .ok_or(CryptoError::Sha256Unavailable)
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
