# Crypto facade validation

Local host: aarch64 macOS, Rust 1.98.1. Branch: `codex/toolkit-security-crypto`,
based on upstream main `dd54fb391b7e5c2ac4217d366470ffad4c395749`.

## Successful checks

| Check | Result |
|---|---|
| toolkit-security, `jwt,internal-auth-cache` | 127 library tests + 1 isolated JWT claims-pass test |
| toolkit-security, `fips,jwt,internal-auth-cache` | 127 library tests + 1 isolated JWT claims-pass test, native corecrypto |
| toolkit-security clippy, both configurations above, all targets, `-D warnings` | Passed |
| toolkit-auth RSA JWKS with `toolkit-security/fips` | 30 passed, including all six independent RS/PS signatures |
| toolkit-http, `fips` | 119 library + 6 integration tests passed |
| toolkit bootstrap, `bootstrap,fips`, crypto conflict integration test | 1 passed |
| toolkit-http-proxy, `fips`, all targets | Compile check passed |
| file-storage | 222 library tests passed, including RFC 8032 seed/key/signature compatibility and existing content hashes |
| orders | 165 passed, 100 existing ignored database tests |
| credstore | 176 passed |
| Firebase / OIDC authentication plugins | 32 / 206 passed |
| toolkit-auth, ordinary configuration | 199 passed |
| construct / GitHub mirror / graph-storage | 326 / 168 / 158 passed |
| OAGW / quota-enforcement SDK | 943 / 96 passed |
| durable execution / types-registry | 246 / 554 passed |
| ledger | 1325 passed, 32 existing ignored database tests |
| BSS approval / REST / pricing SDK / approvals / products | 17 / 41 / 33 / 42 / 427 passed |
| Kubernetes naming | 8 passed |
| Application crypto dependency/source guard | Passed; 4 policy regression tests passed |
| Final locked workspace check, all targets | Passed |
| Formatting and diff whitespace | Passed |

The full workspace, all targets, was checked after migration. This is a compile
check, not a claim that every workspace test or feature combination was run.
The last full check uses `cargo check --offline --locked --workspace --all-targets`.

## Review and limits

The source received a separate read-only design/security review. It covered native
dispatch, HMAC normalization, CNG length limits, RSA component validation and PSS
parameters, JWT curve/hash matching, claims validation and dependency enforcement.
Findings were fixed before the final check. The isolated JWT test replaces the
second-pass verifier with one that panics if verification is invoked. It complements
source inspection of the pinned jsonwebtoken 10.3 RSA/ECDSA constructors; it does not
prove arbitrary constructors are crypto-free. HMAC/EdDSA JWT families are rejected
by this adapter. Re-review constructor behavior when upgrading jsonwebtoken.

Linux AWS-LC FIPS and Windows CNG were not executed on this host. CI now includes
facade/JWT runtime tests on Linux and macOS, and an all-target facade compile check
on Windows. Windows runtime verification still requires a host with system FIPS
policy enabled. CI wiring is not a successful remote CI result.

A combined toolkit-auth + toolkit-http FIPS test invocation failed because the
auth test process did not install a global TLS provider (`NoCryptoProvider`). That
is the HTTP layer's intended fail-closed prerequisite, normally fulfilled by
application bootstrap. Separate native facade/JWKS and HTTP/bootstrap checks above
passed. No production auto-install fallback was introduced to mask the test setup.

File-storage's ordinary Ed25519 token format remains unchanged. Its signing-key
constructors and verification reject Ed25519 in FIPS builds; converting the token
algorithm or migrating keys is outside this change. Upstream/transitive crypto and
Linux/Windows operational-environment validation remain separate work, as described
in the [source audit](crypto-facade-audit.md) and [PRD](PRD.md).
