# Implementation Plan: ToolKit cryptographic facade

Spec: [crypto-facade-spec.md](../docs/security/fips/crypto-facade-spec.md).
Audit: [crypto-facade-audit.md](../docs/security/fips/crypto-facade-audit.md).
Tasks are recorded in [todo.md](todo.md).

## Order

1. Validate the API/backend design and resolve signed-URL compatibility policy.
2. Build SHA-256 facade and per-OS backend selection; test the normal and native FIPS paths.
3. Migrate internal-auth token keys and representative SHA-256 consumers, preserving known digests.
4. Add HMAC-SHA256, constant-time equality and CSPRNG; migrate orders and credstore.
5. Add the chosen signature policy and migrate file-storage, preserving normal-mode keys/tokens.
6. Migrate remaining application hash consumers in small groups; update manifests/lockfile.
7. Centralize TLS provider selection at the facade boundary and propagate FIPS features.
8. Add import/dependency enforcement, reconcile FIPS docs and run consumer/workspace checks.
9. Review the complete diff and record validation limits before committing.

Implementation completed in facade, application migration and policy/documentation commits.
Validation: [crypto-facade-validation.md](../docs/security/fips/crypto-facade-validation.md).
Each implementation slice was verified before final review.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Same digest algorithm but wrong FIPS implementation | Explicit target/feature routing and runtime witness checks, no fallback |
| HMAC long-key or tag behavior changes | Published RFC vectors, long-key normalization tests, full-length verification |
| Ed25519 unavailable in native providers | Explicit user-selected compatibility policy |
| SDK gets a different backend from the server | FIPS feature propagation and standalone facade tests |
| Persisted identifiers or existing tokens change | Known-answer and existing consumer compatibility tests |
| Circular toolkit dependencies | Security facade depends only on backend crates, never toolkit/auth/http |
| Cross-target code compiles but does not run correctly | Record native-host checks and require separate OS runtime validation |
| Mechanical migration wraps TLS internals unnecessarily | Preserve backend implementation boundaries and inventory every exception |
