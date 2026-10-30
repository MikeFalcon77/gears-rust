# ToolKit cryptographic facade

Status: implemented and locally validated on macOS; facade location, per-OS backend policy and preservation of existing signed-URL algorithms agreed with the user on 2026-10-30.
Related: [#3424](https://github.com/constructorfabric/gears-rust/issues/3424).
Base: main `dd54fb391b7e5c2ac4217d366470ffad4c395749`.
Branch: `codex/toolkit-security-crypto`.

## Objective and scope

Make `toolkit_security::crypto` the application entry point for cryptographic primitives.
Migrate the existing application call sites recorded in [the source audit](crypto-facade-audit.md).
Preserve digest/tag/signature bytes, persisted identities, cursors, ETags, and standard-mode tokens.
Backend types remain private; the TLS adapter may expose rustls types because its consumer is rustls.

The first API covers primitives actually used by the workspace: SHA-256, HMAC-SHA256,
cryptographic random bytes, constant-time equality, RSA JWT verification and existing signed-URL signatures.
The optional JWT adapter also supports ECDSA verification for the existing OIDC consumer.
Do not add unused AES/HKDF or algorithm-agility APIs merely to mirror a dependency.

## Backend policy

| Build | Application primitives | TLS adapter |
|---|---|---|
| ordinary build | AWS-LC | existing AWS-LC rustls provider |
| FIPS + Linux | AWS-LC FIPS | AWS-LC FIPS rustls provider |
| FIPS + macOS | existing corecrypto provider | existing corecrypto rustls provider |
| FIPS + Windows | existing CNG provider | existing CNG rustls provider |

No FIPS operation may silently fall back to an ordinary implementation. Reuse existing
runtime witnesses and operational-environment checks; document the limits of those checks.
Feature propagation must reach the security facade when FIPS is enabled through toolkit,
toolkit-http or another standalone facade consumer. A FIPS feature is not itself a certificate.

## API contract

- `crypto::hash::Sha256`: `new`, `update`, `finalize -> [u8; 32]`, and one-shot `digest`.
  Streaming state supports the usage actually needed by consumers; no arbitrary algorithm parameter.
- `crypto::mac::HmacSha256`: owns secret key state, signs bytes, verifies full-length tags in
  constant time, redacts debug output and zeroizes key material. Preserve existing key semantics.
- `crypto::rng::fill`: fills caller-owned bytes using the selected provider and returns errors.
- `crypto::constant_time_eq`: compares contents without data-dependent early exit; length is public.
- `crypto::signature`: owned signed-URL key and verifier interfaces. Existing 32-byte Ed25519 seeds,
  32-byte public keys and 64-byte signatures remain compatible outside FIPS mode.
- `crypto::signature::verify_rsa_components`: exact RS/PS SHA-256/384/512 verification over original
  message bytes; canonical n/e, odd modulus 2048–8192 bits, odd exponent 3..2^33−1, no fallback.
- `crypto::jwt` (optional feature): verify signatures through the facade, then use the original
  jsonwebtoken claim policy. Supports RSA and ES256/384 only; unused HMAC/EdDSA JWT families are rejected. The second pass
  disables only signature verification after it has already succeeded; it never disables claims.
  A separate-process poison-provider test proves jsonwebtoken does not execute another crypto
  operation in that pass. Re-review this assumption on jsonwebtoken upgrades.
- `crypto::tls`: provider factory/state adapter for bootstrap and TLS consumers. TLS implementation
  traits stay at the backend boundary rather than being mechanically renamed.

Expected application style:

```rust
use toolkit_security::crypto::hash::Sha256;

let fingerprint = Sha256::digest(canonical_bytes);
let mut hash = Sha256::new();
hash.update(header);
hash.update(body);
let streamed_fingerprint = hash.finalize();
```

## Architecture

Reuse the existing platform rustls providers through their public primitive interfaces,
avoiding duplicate unsafe OS bindings. SHA-256 is available through the TLS 1.3 suite's
`common.hash_provider`; CSPRNG through `secure_random`. The public `Hkdf::hmac_sign`
contract explicitly signs an arbitrary message using the supplied HMAC key. For SHA-256,
normalize keys longer than the 64-byte SHA-256 block size through the same selected SHA-256
implementation, then use `OkmBlock` (capacity 64) for the normalized key. This normalization
is RFC 2104 HMAC key processing, not a custom cryptographic construction.

A bounded design review confirmed the HMAC operation and found four required policies:
explicit FIPS feature forwarding, rejecting HMAC keys shorter than 14 bytes in FIPS mode,
chunking SHA/RNG buffers at the CNG u32 ABI boundary, and a documented u32::MAX HMAC
message bound with an explicit error. Provider status is an implementation-routing witness,
not a blanket claim about arbitrary operation parameters or deployment environment.
`OkmBlock` zeroizes stored key bytes; facade-owned normalization buffers must also be zeroized.
The rustls hash adapter cannot guarantee erasure of every backend/intermediate copy.

## Source layout and conventions

- Facade: `libs/toolkit-security/src/crypto/`, exported from `src/lib.rs`.
- Backend selection stays private to the facade.
- Tests are separate `*_tests.rs` modules, with independent published known-answer vectors.
- Existing rustfmt, clippy and workspace lint settings apply. No new unsafe bindings in toolkit-security.
- Plan/checklist: `tasks/plan.md` and `tasks/todo.md`.
- Decision/routing documentation: `docs/security/fips/` and the security crate README.

## Testing and commands

```sh
cargo test --locked -p cf-gears-toolkit-security --lib
cargo test --locked -p cf-gears-toolkit-security --lib --features fips,internal-auth-cache
cargo test --locked -p cf-gears-file-storage
cargo check --locked --workspace --all-targets
cargo clippy --locked -p cf-gears-toolkit-security --all-targets -- -D warnings
cargo fmt --all -- --check
```

Use published SHA-256, HMAC-SHA256 and Ed25519 vectors; test streaming chunk boundaries,
empty input, long HMAC keys, wrong/truncated tags, malformed public keys/signatures/seeds,
CSPRNG error propagation, missing FIPS witnesses, and feature/backend selection.
Run consumer tests in bounded groups after each migration slice. Cross-platform checks
must distinguish compile checks from actual Windows/Linux runtime validation.

## Boundaries and success criteria

Always preserve stored/wire bytes and current application semantics. Remove obsolete direct
crypto dependencies from migrated consumer manifests and regenerate Cargo.lock with Cargo.
Keep backend implementations, deliberate independent test oracles and non-security jitter
explicit in the audit. Add an automated guard against reintroducing direct application
primitive dependencies/imports outside the facade.

Never claim cross-platform runtime verification based on a macOS-only run, equate an
approved algorithm with a validated implementation, or silently downgrade FIPS operations.

Completion means application primitive callers use the facade, file-storage no longer
uses ring directly, token-cache SHA-256 follows backend policy, TLS selection remains
consistent, verification passes and remaining backend/test exceptions are documented.

## Signed-URL compatibility policy

The user requested only a facade migration for file-storage, preserving its existing
algorithm and key/token format. Ed25519 remains available outside FIPS; constructors and
verification fail explicitly with UnsupportedAlgorithm under FIPS, without an AWS-LC fallback.
No ECDSA or key/token migration is included.

## JWT compatibility and encoding

The general JWT adapter accepts canonical PKCS#1 RSA public keys and rsaEncryption SPKI;
it rejects arbitrary SPKI algorithm constraints instead of discarding them. Both DER and JWK
components use the same key validator. The toolkit-auth JWKS provider remains RSA-only.
The OIDC and Firebase plugins use the adapter so their original expiration, issuer, audience,
required-claim and leeway checks continue to run in jsonwebtoken.

JOSE ECDSA signatures use fixed-width r||s, converted structurally to DER for the native
verifier; select both the exact curve and hash identifiers rather than the wider TLS 1.2
compatibility mapping. Signature base64 decoding is strict. RSA/ECDSA messages also respect
the CNG u32 limit. Claims decoding remains inside the verified adapter; no public
signature-disabled claims helper exists.

Validation results and unrun OS checks: [crypto-facade-validation.md](crypto-facade-validation.md).
