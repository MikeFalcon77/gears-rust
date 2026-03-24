---
status: proposed
date: 2026-03-23
owner: SDK architecture team
scope: modules/serverless-sdk
---
<!--
 =============================================================================
 ARCHITECTURE DECISION RECORD (ADR) — based on MADR format
 =============================================================================
 RULES:
  - ADRs represent actual decision dilemma and decision state
  - DESIGN is the primary artifact ("what"); ADRs annotate DESIGN with rationale ("why")
  - Use single ADR per decision
 ==============================================================================
 -->

# ADR — Synchronous Environment Interface (Adapter Pre-Fetch Model)


<!-- toc -->

- [Context and Problem Statement](#context-and-problem-statement)
- [Decision Drivers](#decision-drivers)
- [Considered Options](#considered-options)
- [Decision Outcome](#decision-outcome)
  - [Consequences](#consequences)
  - [Confirmation](#confirmation)
- [Pros and Cons of the Options](#pros-and-cons-of-the-options)
  - [Option A: Synchronous Environment](#option-a-synchronous-environment)
  - [Option B: Async Environment](#option-b-async-environment)
  - [Option C: Sync with Optional Async Reload](#option-c-sync-with-optional-async-reload)
- [More Information](#more-information)
- [Non-Applicable Domains](#non-applicable-domains)
- [Traceability](#traceability)

<!-- /toc -->

**ID**: `cpt-cf-serverless-sdk-core-adr-sync-environment`
## Context and Problem Statement

`Handler::call` receives an `&dyn Environment` that provides access to configuration
values and secrets. Secrets on the CyberFabric platform are managed by credstore, and
resolving them requires an async call to the credstore SDK. The `Environment` trait
must decide whether `get_secret` (and `get_config`) are synchronous or asynchronous.

If sync: the adapter must pre-fetch all required values before calling the handler.
If async: the handler can fetch on demand, but the trait becomes async and the
`Environment` object must hold a live credstore client reference across the handler call.

## Decision Drivers

* Handler implementations must remain free of async infrastructure boilerplate
  (no credstore imports, no async fetching setup inside `call`).
* The `Environment` trait must satisfy `cpt-cf-serverless-sdk-core-principle-impl-agnostic`:
  it cannot expose credstore SDK types or any engine-specific interface.
* The `Environment` trait is a testability boundary — unit tests must supply a simple
  `HashMap`-backed mock without any platform infrastructure.
* Secrets needed by a function are declared in the function definition's deployment
  configuration, so the set of secrets required for a given invocation is known before
  the handler is called; eager loading is feasible.
* Most functions require a small number of secrets (typically fewer than 10); eager loading
  has negligible latency impact in the context of a network-bound handler invocation.

## Considered Options

* **Option A**: Synchronous `Environment` — adapter resolves all required values before
  calling the handler and provides them via a `HashMap`-backed implementation.
* **Option B**: Async `Environment` — `get_secret` returns `impl Future<Output = Option<String>>`
  or uses `async-trait`; the trait holds a live credstore client.
* **Option C**: Sync `Environment` with async `reload` — `get_secret` is sync for normal use;
  a separate `async fn reload(&mut self, keys: &[&str])` method allows mid-invocation refresh
  for long-running workflows. The `reload` method is a separate optional trait extension.

## Decision Outcome

Chosen option: **Option A: Synchronous `Environment`**, because the handler authoring
experience is cleanest when `get_secret` and `get_config` are simple `-> Option<&str>` calls
with no async overhead or error handling. The pre-fetch model is workable: the function's
required secrets are known ahead of time from its deployment configuration, and adapters
already perform setup work before calling the handler. The testability benefit (simple
`HashMap` mock, no async test harness) is significant. Option C was rejected as premature
optimisation; the mid-invocation reload use case can be addressed if and when a concrete
requirement for it exists.

### Consequences

* Adapters are responsible for resolving all configuration values and secrets declared by
  the function definition before calling `Handler::call`.
* If a secret is unavailable at pre-fetch time, the adapter must fail the invocation before
  the handler is called (returning a `RuntimeErrorPayload` with `NonRetryable` or `Retryable`
  category as appropriate).
* Long-running workflows that need secrets to remain valid across a multi-day suspension
  must be handled by the adapter (e.g., refreshing secrets on resume). The `Environment`
  contract does not address this; it is an adapter concern.
* If a concrete need for mid-invocation async secret refresh arises in future, a separate
  async `SecretProvider` trait can be introduced in the adapter crate without modifying
  `Environment`. This keeps the breaking-change surface small.

### Confirmation

* `Environment` trait definition has no `async fn`, no `impl Future`, and no `async-trait`
  attribute — verified by code inspection.
* A `HashMap<String, String>`-backed `Environment` implementation compiles and satisfies
  the trait in unit tests without any platform infrastructure.
* `get_config` and `get_secret` return `Option<&str>`, not `Option<String>`, to avoid
  allocations on the hot path — verified in `environment.rs`.

## Pros and Cons of the Options

### Option A: Synchronous Environment

The adapter resolves config and secrets eagerly and provides a snapshot `Environment`.

* Good, because handler code is the simplest possible: `env.get_secret("API_KEY")` with
  no `.await`, no error propagation, no async context required.
* Good, because the trait is trivially testable — any `HashMap` satisfies it.
* Good, because the trait definition has no async machinery, no credstore types, no
  lifetime complexity from a held async client.
* Good, because the set of required secrets is known from the function definition, making
  eager loading deterministic.
* Neutral, because adapters incur an extra round-trip to credstore before each invocation
  (or per cold-start, depending on caching strategy). Acceptable: already in async context.
* Bad, because secrets fetched at invocation start may expire mid-execution for very
  long-running workflows. This edge case requires adapter-level handling.

### Option B: Async Environment

`get_secret` returns a `Future`; the `Environment` holds a live credstore client.

* Good, because secrets can be fetched lazily — unused secrets are never resolved.
* Good, because secrets can be refreshed mid-invocation if they expire.
* Bad, because the `Environment` trait must be async, adding `async-trait` complexity or
  RPITIT signature noise to a trait that is primarily used for simple key lookups.
* Bad, because `env.get_secret("KEY").await?` in handler code requires handlers to import
  `async` plumbing — violates the goal of keeping handlers free of platform boilerplate.
* Bad, because the trait must hold a reference to the credstore client, pulling a platform
  infrastructure type into the trait's type signature or bounding `Self`.
* Bad, because unit testing requires an async mock credstore client, not just a `HashMap`.

### Option C: Sync with Optional Async Reload

`get_secret` is sync; a separate `async fn reload` method refreshes specific keys.

* Good, because normal handler use is sync; the async path is opt-in.
* Good, because it handles the mid-invocation expiry case without forcing async everywhere.
* Bad, because it complicates the trait definition with an orthogonal async extension method.
* Bad, because `reload` requires the `Environment` to hold a mutable async client internally,
  reintroducing the credstore coupling in the trait's implementation surface.
* Bad, because the need for mid-invocation secret refresh has no concrete requirement yet —
  adding this complexity is premature.

## More Information

The pre-fetch model is consistent with how AWS Lambda, Google Cloud Functions, and other
FaaS platforms handle secrets — credentials are loaded at cold start or at invocation setup
time and cached for the duration of the invocation. This is a well-established pattern with
understood trade-offs.

## Non-Applicable Domains

The following checklist domains are not applicable to this ADR and are explicitly excluded:

| Domain | Disposition | Reasoning |
|--------|-------------|-----------|
| SEC | N/A | Decision concerns trait synchrony model; no credential storage in the SDK — credentials are opaque strings passed as `Option<&str>`; security of the underlying credstore is an adapter/platform concern |
| REL | N/A | No stateful data, SLO, or availability commitment at the SDK trait level |
| DATA | N/A | No data schema, persistence, or lifecycle managed by the `Environment` trait |
| OPS | N/A | Pure library; no deployment topology, monitoring infra, or operational concern |
| COMPL | N/A | Internal developer tooling; no regulatory, certification, or legal requirement |
| UX | N/A | No end-user UI; developer ergonomics are the primary driver and are addressed in Decision Outcome |

## Traceability

- **PRD**: [PRD.md](../PRD.md)
- **DESIGN**: [DESIGN.md](../DESIGN.md)

This decision directly addresses the following requirements and design elements:

* `cpt-cf-serverless-sdk-core-fr-environment-trait` — `Environment` trait signature
* `cpt-cf-serverless-sdk-core-principle-impl-agnostic` — no credstore types in the trait
* `cpt-cf-serverless-sdk-core-component-environment` — responsibility boundary definition
