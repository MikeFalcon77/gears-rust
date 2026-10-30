# Crypto facade source audit

Snapshot: `constructorfabric/gears-rust` main `dd54fb391b7e5c2ac4217d366470ffad4c395749`, 2026-10-30.

Related: [#3424](https://github.com/constructorfabric/gears-rust/issues/3424).

The implemented application entry point is `toolkit_security::crypto`, implemented in
`libs/toolkit-security/src/crypto/`. Before this change, the security crate exposed authentication and
authorization interfaces, without a cryptographic facade. Source locations below are the pre-migration snapshot.

## Required application capabilities

| Capability | Current callers | Compatibility requirement |
|---|---|---|
| SHA-256, one-shot and streaming | BSS, graph storage, types registry, GitHub mirror, durable execution, file storage, internal-auth token cache, Kubernetes object names | Preserve digests, cursors, ETags, cache keys and canonical encodings byte for byte |
| HMAC-SHA256 | Orders audit chain, credstore fencing | Preserve tags, key handling and constant-time verification |
| Cryptographic random bytes | Credstore fencing, Ed25519 key generation | Use a CSPRNG and propagate failures |
| Ed25519 signing and verification | File-storage signed URLs | Preserve 32-byte seeds/public keys, 64-byte signatures and existing tokens; verify RFC 8032 vectors |
| Constant-time equality | File-storage finalize authentication, security shared-secret authentication | Preserve matching/missing/invalid credential behavior |

## Boundaries that need explicit treatment

- `rustls::crypto` includes TLS trait implementations, provider assembly, provider installation,
  and tests asserting provider state. These are integration boundaries, not all direct application
  cryptographic operations. Do not mechanically replace the interfaces a provider implements.
- `rand` callers are primarily retry jitter, backoff and coordination nonces. Inventory them,
  but do not relabel them as security randomness. A non-cryptographic API may centralize them
  without promising FIPS properties.
- RSA and SHA-2 in `rustls-corecrypto-provider` and the Firebase plugin include independent
  test fixtures. Keep independent oracles where they validate a provider implementation.
- `toolkit-auth` calls `jsonwebtoken::crypto::verify`; JWT parsing/key-format handling and
  algorithm allow-list enforcement need an explicit adapter decision.
- FIPS validates a module/implementation in its specified environment, not the algorithm name.
  An AWS-LC wrapper alone does not establish cross-platform FIPS routing. Current TLS policy
  selects AWS-LC FIPS on Linux, corecrypto on macOS and CNG on Windows.
- `file-storage/src/infra/content/hash.rs` declares content hashes outside its FIPS claim;
  a facade migration can preserve the hashes while making implementation selection explicit.

## Source locations

### Application AWS-LC primitives

33 files matched (including comments and tests).

- `gears/bss/approvals/approvals/src/domain/cursor.rs`: lines 5
- `gears/bss/ledger/ledger/src/domain/canonical.rs`: lines 11
- `gears/bss/ledger/ledger/src/domain/invoice/reversal.rs`: lines 23
- `gears/bss/ledger/ledger/src/infra/posting/idempotency.rs`: lines 15
- `gears/bss/libs/approval/src/hash.rs`: lines 4
- `gears/bss/libs/rest/src/conditional_get.rs`: lines 6
- `gears/bss/orders/orders/src/domain/audit.rs`: lines 10
- `gears/bss/orders/orders/src/domain/idempotency.rs`: lines 312
- `gears/bss/orders/orders/src/domain/read.rs`: lines 10
- `gears/bss/orders/orders/tests/conformance_vectors.rs`: lines 47
- `gears/bss/pricing/pricing-sdk/src/digest.rs`: lines 14
- `gears/bss/pricing/pricing/src/api/rest/preconditions.rs`: lines 261
- `gears/bss/products/products/src/domain/canonical.rs`: lines 30
- `gears/bss/products/products/src/domain/derived.rs`: lines 27
- `gears/construct/construct/src/domain/record_intake.rs`: lines 135
- `gears/credstore/credstore/src/domain/secret/fence.rs`: lines 22, 70, 85
- `gears/github-mirror/github-mirror/src/domain/sync/change_gate.rs`: lines 4
- `gears/github-mirror/github-mirror/src/infra/github/cache.rs`: lines 17
- `gears/github-mirror/github-mirror/src/infra/github/compression.rs`: lines 14
- `gears/github-mirror/github-mirror/src/infra/github/rate_limit/registry.rs`: lines 7
- `gears/graph-storage/graph-storage-sdk/src/models.rs`: lines 1434
- `gears/graph-storage/graph-storage/src/domain/embedding.rs`: lines 16
- `gears/graph-storage/graph-storage/src/domain/identity.rs`: lines 7
- `gears/graph-storage/graph-storage/src/infra/embedding/fake.rs`: lines 17
- `gears/graph-storage/onnx-embedding-plugin/src/lib.rs`: lines 30
- `gears/system/oagw/oagw/src/domain/services/registry_state.rs`: lines 17
- `gears/system/quota-enforcement/quota-enforcement-sdk/src/models.rs`: lines 16
- `gears/system/quota-enforcement/quota-enforcement-sdk/src/models_tests.rs`: lines 795
- `gears/system/types-registry/types-registry/src/domain/admission/fingerprint.rs`: lines 51
- `gears/system/types-registry/types-registry/src/domain/artifacts.rs`: lines 20
- `gears/system/types-registry/types-registry/src/domain/validator.rs`: lines 3
- `libs/toolkit-durable-execution/src/contracts.rs`: lines 10
- `libs/toolkit-durable-execution/src/store/start.rs`: lines 13

### RustCrypto SHA-2

5 files matched (including comments and tests).

- `gears/file-storage/file-storage/src/infra/content/hash.rs`: lines 12
- `gears/file-storage/file-storage/src/infra/content/hash_mode_tests.rs`: lines 3
- `gears/system/cluster/plugins/k8s-cluster-plugin/src/naming.rs`: lines 13
- `libs/rustls-corecrypto-provider/src/signer/rsa_tests.rs`: lines 257
- `libs/toolkit-security/src/internal_auth_cache.rs`: lines 120

### ring primitives

2 files matched (including comments and tests).

- `gears/file-storage/file-storage/src/api/rest/handlers.rs`: lines 96
- `gears/file-storage/file-storage/src/infra/signed_url/provider.rs`: lines 12, 13

### rand (inspect purpose before migrating)

22 files matched (including comments and tests).

- `gears/credstore/credstore/src/domain/secret/service.rs`: lines 503, 504
- `gears/system/account-management/plugins/keycloak-idp-plugin/src/infra/kc_http.rs`: lines 218, 219
- `gears/system/cluster/cluster-sdk/src/restart.rs`: lines 29, 144
- `gears/system/cluster/cluster/src/defaults/leader.rs`: lines 7, 156, 160, 913
- `gears/system/cluster/cluster/src/defaults/lease.rs`: lines 123, 124
- `gears/system/cluster/cluster/src/defaults/lease_tests.rs`: lines 42, 46, 713, 717
- `gears/system/cluster/cluster/src/defaults/lock.rs`: lines 93, 97, 341, 345
- `gears/system/cluster/plugins/k8s-cluster-plugin/src/backoff.rs`: lines 16, 21
- `gears/system/cluster/plugins/k8s-cluster-plugin/src/lease.rs`: lines 74, 75
- `gears/system/cluster/plugins/postgres-cluster-plugin/src/cache/watch.rs`: lines 32, 665
- `gears/system/cluster/plugins/redis-cluster-plugin/src/lock/waiters.rs`: lines 47, 104
- `gears/system/event-broker/event-broker/src/infra/loader/heads.rs`: lines 174
- `gears/system/event-broker/event-broker/src/infra/storage/facade.rs`: lines 571
- `gears/system/event-broker/event-broker/src/infra/storage/producer_chain.rs`: lines 157
- `gears/system/event-broker/plugins/mysql-event-broker-plugin/src/startup.rs`: lines 11, 75
- `gears/system/event-broker/plugins/postgres-event-broker-plugin/src/startup.rs`: lines 11, 63
- `gears/system/types-registry/types-registry-sdk/src/reconcile/submit.rs`: lines 113, 116
- `gears/system/usage-collector/plugins/timescaledb-usage-collector-plugin/src/infra/storage/record_store.rs`: lines 20, 735
- `libs/toolkit-auth/src/oauth2/token_watcher.rs`: lines 21, 433
- `libs/toolkit-contract/src/runtime/client.rs`: lines 977, 984
- `libs/toolkit-contract/src/runtime/retry.rs`: lines 6, 96
- `libs/toolkit-http/src/layers/retry.rs`: lines 7, 418

### rustls provider integration and implementations

27 files matched (including comments and tests).

- `gears/system/oagw/oagw/src/domain/test_support.rs`: lines 40, 63
- `gears/system/usage-collector/plugins/clickhouse-usage-collector-plugin/src/gear_tests.rs`: lines 41
- `gears/system/usage-collector/plugins/clickhouse-usage-collector-plugin/src/infra/storage/pool.rs`: lines 155
- `gears/system/usage-collector/plugins/clickhouse-usage-collector-plugin/src/infra/storage/pool_tests.rs`: lines 25
- `gears/system/usage-collector/plugins/clickhouse-usage-collector-plugin/tests/common/mod.rs`: lines 96
- `gears/system/usage-collector/plugins/clickhouse-usage-collector-plugin/tests/crypto_provider_required.rs`: lines 9, 34
- `libs/rustls-corecrypto-provider/src/hash.rs`: lines 1, 18
- `libs/rustls-corecrypto-provider/src/hkdf.rs`: lines 1, 7, 8, 9
- `libs/rustls-corecrypto-provider/src/hmac.rs`: lines 1, 11
- `libs/rustls-corecrypto-provider/src/kx.rs`: lines 12
- `libs/rustls-corecrypto-provider/src/lib.rs`: lines 1, 67, 75
- `libs/rustls-corecrypto-provider/src/provider.rs`: lines 1, 16, 17, 18
- `libs/rustls-corecrypto-provider/src/random.rs`: lines 6
- `libs/rustls-corecrypto-provider/src/signer/mod.rs`: lines 3, 18
- `libs/rustls-corecrypto-provider/src/tls12.rs`: lines 21, 22, 27, 127, 144
- `libs/rustls-corecrypto-provider/src/tls12_tests.rs`: lines 2, 298
- `libs/rustls-corecrypto-provider/src/tls13.rs`: lines 19
- `libs/rustls-corecrypto-provider/src/tls13_tests.rs`: lines 2
- `libs/rustls-corecrypto-provider/src/verify.rs`: lines 1, 4, 21
- `libs/rustls-corecrypto-provider/tests/handshake_smoke.rs`: lines 33, 53, 67
- `libs/toolkit-http-proxy/src/tls.rs`: lines 133
- `libs/toolkit-http-proxy/tests/fips.rs`: lines 19
- `libs/toolkit-http-proxy/tests/support/mod.rs`: lines 49, 52, 55, 57
- `libs/toolkit-http/src/tls.rs`: lines 96, 97, 132, 136, 149, 206, 250
- `libs/toolkit-http/tests/no_crypto_provider_fips.rs`: lines 21
- `libs/toolkit/src/bootstrap/crypto.rs`: lines 29, 169, 181, 193
- `libs/toolkit/tests/crypto_init_fips.rs`: lines 10

### JWT adapter

- `libs/toolkit-auth/src/providers/jwks.rs`: verification at line 369, test signing at line 1216.

## Verification already completed

- Before backend migration, 14 file-storage signed-URL tests pass, including a newly added
  RFC 8032 Ed25519 known-answer test and rejection of invalid seed lengths.

## Open design decision

The user selected immediate per-OS application FIPS routing. The remaining compatibility
decision is how file-storage signatures behave where the native provider lacks Ed25519;
see [the specification](crypto-facade-spec.md).

## Migration result

Application SHA-256, orders/credstore HMAC, credstore CSPRNG, file-storage Ed25519
and finalize constant-time comparison now use the facade. Direct ring and sha2 dependencies
were removed from application manifests. The final inverse dependency tree still
contains `ring 0.17.14` via rustls/sqlx-core and ureq build dependencies. Backend selection for bootstrap/HTTP and TLS
test setup now calls the same facade factory; existing TLS trait implementations remain.

The follow-up high-level scan also found implicit verification in
`oidc-authn-plugin/src/domain/validator.rs` and `firebase-authn-plugin/src/firebase.rs`.
Both now use `crypto::jwt::decode`; toolkit-auth's direct RSA verification uses
`crypto::signature::verify_rsa_components`. No production JWT signature operation
runs through jsonwebtoken's AWS-LC verifier. Its existing claims validation is preserved.

Remaining exceptions:

- `crypto/signature.rs`: ordinary Ed25519 AWS-LC implementation; explicitly unavailable under FIPS.
- `crypto/backend.rs`: canonical rustls/native provider factory.
- `rustls-corecrypto-provider`: native TLS implementation, plus dev-only RSA/SHA-2 independent oracle.
- Firebase dev-only RSA fixture generation; JWT signing in tests is an independent AWS-LC oracle.
- `toolkit/tests/crypto_init_fips.rs`: deliberately installs the wrong provider to test conflicts.
- Retry jitter, coordination nonces and random outbox candidate IDs remain non-security `rand` uses.
- Transitive crypto in SDKs/third-party libraries is still Phase B work; linking a crate is different
  from executing application crypto through it. This change does not complete that dependency purge.

`tools/scripts/check_crypto_facade.py` checks resolved direct dependencies (including aliases,
target sections, build and dev dependencies) and common direct TLS/JWT bypasses. It is an
application regression guard, not Rust type analysis or proof about arbitrary re-exports.
The only dependency exceptions are the facade backend and named independent dev oracles.
