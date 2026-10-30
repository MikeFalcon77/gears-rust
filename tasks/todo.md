# ToolKit crypto implementation tasks

- [x] Create `codex/toolkit-security-crypto` from current upstream main.
- [x] Inventory direct application primitive use, TLS integration and test oracles.
- [x] Establish an Ed25519 compatibility baseline (14 signed-URL tests pass).
- [x] Confirm immediate per-OS FIPS routing with the user.
- [x] Resolve signed-URL FIPS compatibility policy: facade migration only; preserve Ed25519 and refuse unsupported FIPS operations.
- [x] Validate the facade/backend API through a bounded design review.
- [x] Implement/test SHA-256 and per-OS selection (security crate, private backend, test module).
- [x] Migrate/test internal-auth credential digests (security crate feature, token cache).
- [x] Implement/test HMAC-SHA256 and CSPRNG (facade modules, known-answer tests).
- [x] Migrate/test orders audit MAC and credstore fencing/randomness in separate slices.
- [x] Implement/test signature policy; migrate file-storage and constant-time authentication.
- [x] Migrate/test BSS SHA-256 consumers in bounded package groups.
- [x] Migrate/test graph storage, types registry and quota-enforcement digest consumers.
- [x] Migrate/test GitHub mirror, construct, OAGW and durable execution digest consumers.
- [x] Migrate/test Kubernetes naming digests; preserve exact existing object names.
- [x] Route toolkit-auth, OIDC and Firebase JWT verification through the native facade while preserving claims validation.
- [x] Route TLS provider selection consistently and forward FIPS features.
- [x] Enforce facade usage; update dependency policy and source audit exceptions.
- [x] Regenerate/review Cargo.lock; run formatting, checks and meaningful consumer tests.
- [x] Review the complete change, document unrun OS checks and commit the verified result.

Validation evidence and OS limits: [crypto-facade-validation.md](../docs/security/fips/crypto-facade-validation.md).
