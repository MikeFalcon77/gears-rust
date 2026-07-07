<!-- @cpt-dod:cpt-cf-clst-dod-showcase-audit-traceability:p1 -->
# TRACEABILITY AUDIT — Cluster SDK

- [x] `p1` - **ID**: `cpt-cf-clst-algo-showcase-audit-traceability`

Pre-archive traceability audit for the cluster SDK change (feature
`cpt-cf-clst-feature-showcase-audit`, DECOMPOSITION §2.12). It verifies that
every PRD requirement maps to a realizing DESIGN section or ADR **and** to a
feature, confirms the code traceability markers are wired, and records the
resolution of the two open questions.

<!-- toc -->

- [1. Method](#1-method)
- [2. Requirement → DESIGN/ADR → Feature](#2-requirement--designadr--feature)
- [3. Principles & Constraints → DESIGN/ADR → Feature](#3-principles--constraints--designadr--feature)
- [4. Code Marker Verification](#4-code-marker-verification)
- [5. Open Questions (DESIGN §7)](#5-open-questions-design-7)
- [6. Conclusion](#6-conclusion)

<!-- /toc -->

## 1. Method

- **Requirement source**: the 38 functional/non-functional requirements declared
  in [PRD.md](PRD.md) (`cpt-cf-clst-fr-*`, `cpt-cf-clst-nfr-*`).
- **Realization source**: [DESIGN.md](DESIGN.md) §3 subsections and the ten ADRs
  under [ADR/](ADR/).
- **Feature source**: the assignment recorded in [DECOMPOSITION.md](DECOMPOSITION.md)
  §2 ("Requirements Covered" per feature).
- **Marker source**: `@cpt-dod:` markers grepped from `cluster-sdk/src`,
  `cluster-sdk/tests`, `cluster/examples`, and `tools/dylint_lints/de14_cluster`.
- **Scope key**: `code` = realized by this change's shipped code; `follow-up` =
  enabling contract shipped here, full realization deferred to the wiring crate /
  parent host gear per PRD §4.1 (each still maps to a realizing ADR/DESIGN section,
  so traceability is complete and implementation is staged).

## 2. Requirement → DESIGN/ADR → Feature

| Requirement | Realizing DESIGN / ADR | Feature | Scope |
|---|---|---|---|
| `cpt-cf-clst-fr-cache-storage` | §3.1, §3.3; ADR-001 | 02 cache-primitive | code |
| `cpt-cf-clst-fr-cache-atomic` | §3.3; ADR-001; principle version-based-cas | 02 cache-primitive | code |
| `cpt-cf-clst-fr-cache-ttl` | §3.3 | 02 cache-primitive | code |
| `cpt-cf-clst-fr-cache-watch` | §3.9; ADR-003 | 02 cache-primitive | code |
| `cpt-cf-clst-fr-leader-elect` | §3.1, §3.3; ADR-009 | 03 leader-election | code |
| `cpt-cf-clst-fr-leader-config` | §3.1; ADR-009 | 03 leader-election | code |
| `cpt-cf-clst-fr-leader-observability` | §3.9; ADR-003 | 03 leader-election | code |
| `cpt-cf-clst-fr-leader-resign` | §3.3, §3.7 | 03 leader-election | code |
| `cpt-cf-clst-fr-leader-advisory` | §3.3; ADR-009 | 03 leader-election | code |
| `cpt-cf-clst-fr-lock-acquire` | §3.3 | 04 distributed-lock | code |
| `cpt-cf-clst-fr-lock-release` | §3.3, §3.7 | 04 distributed-lock | code |
| `cpt-cf-clst-fr-lock-no-remote` | ADR-002; constraint no-remote-in-critical-section | 10 lock-lint | code |
| `cpt-cf-clst-fr-sd-register` | §3.1, §3.3 | 05 service-discovery | code |
| `cpt-cf-clst-fr-sd-discover` | §3.1, §3.10 | 05 service-discovery | code |
| `cpt-cf-clst-fr-sd-watch` | §3.9 | 05 service-discovery | code |
| `cpt-cf-clst-fr-sd-state` | ADR-008 | 05 service-discovery | code |
| `cpt-cf-clst-fr-namespacing-scoped` | §3.8 | 07 scoping-polyfill | code |
| `cpt-cf-clst-fr-namespacing-sd-metadata-unscoped` | §3.8; ADR-008 | 07 scoping-polyfill | code |
| `cpt-cf-clst-fr-routing-cache-only-plugin` | §3.11; ADR-001 | 06 sdk-default-backends | code |
| `cpt-cf-clst-fr-validation-typed-profile` | §3.6; ADR-007 | 01 sdk-foundation | code |
| `cpt-cf-clst-fr-validation-capability-declarations` | §3.10; ADR-007 | 02 cache-primitive | code |
| `cpt-cf-clst-fr-validation-honest-declaration` | §3.10; ADR-007 | 02 cache-primitive | code |
| `cpt-cf-clst-fr-validation-startup-fail` | §3.6, §3.10; ADR-007 | 02 cache-primitive | code |
| `cpt-cf-clst-fr-watch-auto-restart` | §3.9; ADR-003 | 08 watch-auto-restart | code |
| `cpt-cf-clst-fr-watch-lifecycle-signals` | §3.9; ADR-003 (shutdown delivery: ADR-006) | 02 cache-primitive | code |
| `cpt-cf-clst-nfr-error-retryability` | §3.9; ADR-003 | 01 sdk-foundation | code |
| `cpt-cf-clst-nfr-plugin-stability` | §3.2, §3.5; ADR-005; constraint dyn-compat | 01 sdk-foundation | code |
| `cpt-cf-clst-nfr-capability-validation` | §3.10; ADR-007 | 02 cache-primitive | code |
| `cpt-cf-clst-nfr-watch-delivery` | §3.9; ADR-003 | 02 cache-primitive | code |
| `cpt-cf-clst-nfr-leader-guarantee` | §3.11; ADR-001, ADR-009 | 06 sdk-default-backends | code |
| `cpt-cf-clst-nfr-bounded-critical-section` | ADR-002 | 10 lock-lint | code |
| `cpt-cf-clst-nfr-observability` | §3.2; ADR-004; [OBSERVABILITY.md](OBSERVABILITY.md) | 09 registration-observability | code |
| `cpt-cf-clst-nfr-cross-backend-stability` | §6; smoke-test baseline | 11 smoke-tests | code |
| `cpt-cf-clst-fr-routing-per-primitive` | §3.2, §3.13; ADR-006 | `cluster/src/wiring.rs` (`reject_unsupported_native_bindings`) | follow-up (native non-cache binding rejected at config time) |
| `cpt-cf-clst-fr-routing-omit-default` | §3.11; ADR-001, ADR-006 | `cluster/src/wiring.rs` (`build_and_start` auto-fill) | code |
| `cpt-cf-clst-fr-lifecycle-owner` | §3.7, §3.13; ADR-006 | `cluster/src/gear.rs`, `cluster/src/wiring.rs` | code |
| `cpt-cf-clst-fr-shutdown-revoke` | §3.13; ADR-006 | `cluster/src/wiring.rs` (`ClusterHandle::stop`), `cluster-sdk/src/defaults/leader.rs`, `cluster-sdk/src/defaults/lock.rs`, `cluster-sdk/src/defaults/discovery.rs` (`ShutdownRevoke`), `plugins/standalone-cluster-plugin/src/cache.rs` (`StandaloneCache::shutdown`) | code |
| `cpt-cf-clst-fr-shutdown-ttl-cleanup` | §3.13; ADR-006 | `cluster/src/wiring.rs` (`ClusterHandle::stop`) | code |

**Coverage**: 38/38 requirements map to a realizing DESIGN section or ADR and to
a feature or realizing code. The only remaining follow-up is
`cpt-cf-clst-fr-routing-per-primitive` (native non-cache backends per primitive),
which the wiring rejects loudly at config time until those providers ship.
`cpt-cf-clst-fr-shutdown-revoke` is now fully realized (leader, in-flight lock,
service-discovery watch, and cache watch all observe a terminal `Shutdown`). No
orphan requirements.

## 3. Principles & Constraints → DESIGN/ADR → Feature

| Element | Realizing DESIGN / ADR | Feature |
|---|---|---|
| `cpt-cf-clst-principle-cas-universal` | ADR-001; §3.11 | 02 cache-primitive |
| `cpt-cf-clst-principle-facade-plus-backend-trait` | ADR-005; §3.2 | 02 cache-primitive |
| `cpt-cf-clst-principle-lightweight-notifications` | §3.9; ADR-003 | 02 cache-primitive |
| `cpt-cf-clst-principle-version-based-cas` | §3.3; ADR-001 | 02 cache-primitive |
| `cpt-cf-clst-principle-watch-union-shape` | §3.9; ADR-003 | 02 cache-primitive |
| `cpt-cf-clst-principle-per-primitive-routing` | §3.2; ADR-006 | 09 registration-observability |
| `cpt-cf-clst-constraint-no-serde` | §3.5; ADR-005 | 01 sdk-foundation |
| `cpt-cf-clst-constraint-dyn-compat` | §3.5; ADR-005 | 01 sdk-foundation |
| `cpt-cf-clst-constraint-no-remote-in-critical-section` | ADR-002 | 10 lock-lint |

## 4. Code Marker Verification

34 distinct `@cpt-dod:` markers are wired in code across `cluster-sdk/src`,
`cluster-sdk/tests`, `cluster/examples`, and the lint crate; a 35th,
`cpt-cf-clst-dod-showcase-audit-traceability`, is carried by this audit document
itself. Every in-scope feature (01–12) has at least one wired DoD marker:

| Feature | Representative wired DoD markers |
|---|---|
| 01 sdk-foundation | `dod-sdk-foundation-{crate-scaffold,error-model,profile,dyn-compat}` |
| 02 cache-primitive | `dod-cache-primitive-{backend-facade,types,resolver,watch}` |
| 03 leader-election | `dod-leader-election-{backend-facade,config,watch,advisory}` |
| 04 distributed-lock | `dod-distributed-lock-{backend-facade,guard}` |
| 05 service-discovery | `dod-service-discovery-{backend-facade,types,handle,watch}` |
| 06 sdk-default-backends | `dod-sdk-default-backends-{leader,lock,sd}` |
| 07 scoping-polyfill | `dod-scoping-polyfill-{wrappers,polling}` |
| 08 watch-auto-restart | `dod-watch-auto-restart-{combinator,policy}` |
| 09 registration-observability | `dod-registration-observability-{helpers,gts,obs}` |
| 10 lock-lint | `dod-lock-lint-rule` |
| 11 smoke-tests | `dod-smoke-tests-{stubs,resolution,coordination,watch}` |
| 12 showcase-audit | `dod-showcase-audit-examples` (examples), `dod-showcase-audit-traceability` (this doc) |

No in-scope feature is missing its code markers.

## 5. Open Questions (DESIGN §7)

| Question | Resolution |
|---|---|
| Whether ADR-003 (cache watch backpressure) broadens to cover all three watches, or a new ADR captures the generalization | **Resolved.** ADR-003 was generalized on 2026-04-27 — it now carries a "Generalization to all three watches" section covering `LeaderWatch` and `ServiceWatch`, with the lightweight-notifications principle folded in. The decision is unchanged; no separate ADR is needed. This matches the DESIGN §7 recommendation ("broaden ADR-003"). |
| Backend authentication and credential wiring | **Deferred (not a gap).** Owned by the platform OOP deployment design (PRD §4.2 / §7); the SDK contract exposes no authentication or authorization surface. Transport authentication, credential wiring, and tenant isolation are backend/plugin concerns resolved as part of the broader OOP design, out of scope for this change. |

## 6. Conclusion

- Every requirement maps to a realizing DESIGN section or ADR and to a feature
  (§2). Principles and constraints likewise (§3).
- Code traceability markers are wired for every in-scope feature (§4).
- Both open questions are resolved/recorded (§5).
- **No traceability gaps** for this change. The one `follow-up`-scoped
  requirement is intentionally deferred (PRD §4.1) to the wiring crate and
  parent host gear; its realizing ADR (ADR-006) and DESIGN sections exist, so
  the follow-up changes build against a frozen, fully-traced contract.
