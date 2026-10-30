# ToolKit Security

Security context and authorization primitives for Gears / ToolKit.

## Overview

The `cf-gears-toolkit-security` crate provides:

- `SecurityContext`
- `AccessScope`
- Permission / policy engine interfaces
- Binary codec helpers for encoding/decoding security context
- Application cryptography through `toolkit_security::crypto`

## Application cryptography

Use `crypto::hash::Sha256`, `crypto::mac::HmacSha256`, `crypto::rng::fill`,
`crypto::constant_time_eq` and `crypto::signature` instead of backend crates.
The optional `jwt` feature provides `crypto::jwt::decode`, preserving jsonwebtoken
claim validation after verifying signatures through the facade. The explicit TLS
adapter is `crypto::tls::configured_provider`.

Ordinary builds use AWS-LC. With `fips`, Linux uses AWS-LC FIPS, macOS uses the
existing corecrypto provider and Windows uses CNG. `toolkit/fips` and
`toolkit-http/fips` forward this feature. Standalone consumers enable
`toolkit-security/fips` explicitly. Rust features unify across the dependency graph.
Call `crypto::initialize()` at startup to handle unavailable providers before
processing requests; infallible hash methods panic when backend policy cannot be met.

No FIPS operation falls back to an ordinary backend. Ed25519 remains compatible
with existing file-storage seeds and signed URLs outside FIPS; its constructors,
signing and verification return `UnsupportedAlgorithm` under FIPS. RSA verification
supports RS256/384/512 and PS256/384/512. The JWT adapter additionally supports
ES256/384. HMAC/EdDSA JWT families are not currently needed and are rejected.

HMAC FIPS keys must be at least 14 bytes. HMAC and RSA/ECDSA messages are limited to
`u32::MAX` bytes for portable CNG dispatch; SHA-256 and RNG split larger buffers.
Facade-owned HMAC keys/normalization buffers are zeroized; this does not guarantee
erasure of every intermediate copy in rustls or OS APIs.

A provider's `fips()` status is a routing/runtime witness, not deployment certification.
Follow the module certificate's operating-environment and security-policy requirements.
macOS has an existing runtime OS-version check; Linux/Windows operating-environment
coverage still requires release validation, and Windows must enable system FIPS policy.
See [facade design](../../docs/security/fips/crypto-facade-spec.md) and
[source audit](../../docs/security/fips/crypto-facade-audit.md).

## License

Licensed under Apache-2.0.
