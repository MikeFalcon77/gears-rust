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

STANDARDS ALIGNMENT:
 - MADR (Markdown Any Decision Records)
 - IEEE 42010 (architecture decisions as first-class elements)
 - ISO/IEC 15288 / 12207 (decision analysis process)
==============================================================================
-->

# ADR — Use `async-trait` for Handler and WorkflowHandler Traits


<!-- toc -->

- [Context and Problem Statement](#context-and-problem-statement)
- [Decision Drivers](#decision-drivers)
- [Considered Options](#considered-options)
- [Decision Outcome](#decision-outcome)
  - [Consequences](#consequences)
  - [Confirmation](#confirmation)
- [Pros and Cons of the Options](#pros-and-cons-of-the-options)
  - [Option A: `async-trait`](#option-a-async-trait)
  - [Option B: RPITIT with explicit `Send` bound](#option-b-rpitit-with-explicit-send-bound)
  - [Option C: `async fn` without `Send` bound](#option-c-async-fn-without-send-bound)
- [More Information](#more-information)
- [Non-Applicable Domains](#non-applicable-domains)
- [Traceability](#traceability)

<!-- /toc -->

**ID**: `cpt-cf-serverless-sdk-core-adr-async-trait`
## Context and Problem Statement

`Handler<I, O>` and `WorkflowHandler<I, O>` require async methods (`call`, `compensate`)
so handler implementations can perform async I/O (outbound HTTP, secret fetching, etc.).
Rust stable does not allow `async fn` in trait definitions with automatic `+ Send` bounds
on the returned `Future`. Two approaches exist on stable Rust 1.92: the `async-trait`
crate (which boxes the returned `Future`) and Return-Position Impl Trait in Traits
(RPITIT, stable since 1.75) which requires explicit `-> impl Future<...> + Send + '_`
method signatures.

Which approach should be used for the async trait methods in this crate?

## Decision Drivers

* Handler trait implementations must produce `Future` values that are `+ Send`, because
  adapters dispatch handler calls across multi-threaded tokio runtimes.
* The handler authoring experience must be as ergonomic as possible — function authors
  should write `async fn call(...)` without boilerplate.
* The crate must compile on stable Rust 1.92.0 with no nightly features
  (`cpt-cf-serverless-sdk-core-constraint-stable-rust`).
* `async-trait` is already a workspace dependency; RPITIT requires no additional
  dependency but requires more verbose method signatures and explicit lifetime annotations.

## Considered Options

* **Option A**: `async-trait` crate — `#[async_trait]` attribute on trait and impls,
  ergonomic `async fn` syntax, `Box<dyn Future + Send>` expansion under the hood.
* **Option B**: RPITIT with explicit `Send` bound — `fn call<'a>(&'a self, ...) -> impl Future<Output = ...> + Send + 'a` method signatures, no extra dependency, no boxing.
* **Option C**: `async fn` without Send bound — `async fn call(...)` with no `+ Send`
  on the Future; callers must constrain via `where H: Handler<I,O>, H::Future: Send`.
  (Not explored further: requires unstable associated type bounds in practice.)

## Decision Outcome

Chosen option: **Option A: `async-trait`**, because it gives `async fn call(...)` syntax
in both the trait definition and all implementations — the exact ergonomic form function
authors expect — while automatically ensuring `Future + Send` bounds without any
annotation burden on implementors. The boxing overhead is negligible in a context where
each `call` performs async I/O anyway, and `async-trait` is already a workspace
dependency (`cpt-cf-serverless-sdk-core-constraint-stable-rust` is satisfied).

### Consequences

* All `Handler<I, O>` and `WorkflowHandler<I, O>` implementors must annotate their `impl`
  blocks with `#[async_trait]`.
* The returned `Future` from `call` and `compensate` is heap-allocated (`Box<dyn Future>`),
  adding one allocation per invocation. Acceptable because each invocation is a coarse
  unit of work, not a hot inner loop.
* When RPITIT with `Send` bound gains full stable support with ergonomic syntax (likely Rust
  2024 edition improvements), this decision should be revisited to remove the `async-trait`
  dependency. The change would be backward-compatible at the trait level.
* Trait object dispatch (`dyn Handler<I, O>`) is possible without special ergonomics,
  which simplifies adapter type-erased handler registries.

### Confirmation

* `cargo check` on stable 1.92.0 must pass with `#[async_trait]` removed — confirm it fails
  without the attribute (confirming the attribute is load-bearing, not accidental).
* All handler impls in tests and examples must use `#[async_trait]`.
* Code review: no `impl Future` / RPITIT syntax in `handler.rs` or `workflow.rs`.

## Pros and Cons of the Options

### Option A: `async-trait`

Crate attribute expands `async fn` to `fn ... -> Pin<Box<dyn Future + Send>>`.
Already a workspace dependency.

* Good, because implementors write natural `async fn call(...)` — no lifetime annotations.
* Good, because `Future + Send` is guaranteed automatically without per-impl constraints.
* Good, because trait objects `dyn Handler<I, O>` work out of the box.
* Good, because already a workspace dependency — zero new crate surface.
* Neutral, because one heap allocation per invocation (acceptable for coarse-grained I/O work).
* Bad, because implementors must remember `#[async_trait]` on every `impl` block.

### Option B: RPITIT with explicit `Send` bound

```rust
fn call<'a>(
    &'a self,
    ctx: &'a Context,
    env: &'a dyn Environment,
    input: I,
) -> impl Future<Output = Result<O, ServerlessSdkError>> + Send + 'a;
```

* Good, because zero runtime overhead — no boxing.
* Good, because no additional dependency.
* Bad, because each method needs explicit lifetime and `+ Send + 'a` annotation — verbose.
* Bad, because trait objects `dyn Handler<I, O>` are **not** object-safe with RPITIT methods,
  preventing adapters from storing `Box<dyn Handler<I, O>>`.
* Bad, because the explicit lifetime on the return position is non-obvious and error-prone
  for function authors unfamiliar with RPITIT semantics.

### Option C: `async fn` without `Send` bound

```rust
async fn call(&self, ctx: &Context, env: &dyn Environment, input: I)
    -> Result<O, ServerlessSdkError>;
```

* Good, because cleanest author-facing syntax.
* Bad, because the returned `Future` is not `Send` without explicit bounds or workarounds.
* Bad, because adapters running on multi-threaded tokio would need `where H::Future: Send`
  constraints propagated everywhere — shifting burden to adapter code.

## More Information

The `async-trait` crate was authored by David Tolnay and has been stable for multiple years
with wide adoption in the Rust ecosystem. Its boxing behaviour is deterministic and
well-understood. The Rust project has a tracking issue for native async fn in traits with
object safety and `Send` bound support; when that stabilises, this ADR should be revisited.

## Non-Applicable Domains

The following checklist domains are not applicable to this ADR and are explicitly excluded:

| Domain | Disposition | Reasoning |
|--------|-------------|-----------|
| SEC | N/A | Decision concerns trait ergonomics and Rust async machinery; no credential management, no authentication, no user data |
| REL | N/A | No stateful data or availability SLO; library crate with no deployment artifact |
| DATA | N/A | No data storage, schema, or persistence involved in this decision |
| OPS | N/A | Pure library; no deployment topology, monitoring, or infrastructure concern |
| COMPL | N/A | Internal developer tooling; no regulatory, certification, or legal requirement |
| UX | N/A | No end-user UI or user-facing strings; developer ergonomics addressed in Decision Outcome |

## Traceability

- **PRD**: [PRD.md](../PRD.md)
- **DESIGN**: [DESIGN.md](../DESIGN.md)

This decision directly addresses the following requirements and design elements:

* `cpt-cf-serverless-sdk-core-fr-handler-trait` — async `call` method signature
* `cpt-cf-serverless-sdk-core-fr-handler-send-sync` — `Future + Send` guarantee on handler futures
* `cpt-cf-serverless-sdk-core-fr-workflow-handler-trait` — async `compensate` method signature
* `cpt-cf-serverless-sdk-core-constraint-stable-rust` — no nightly features required
* `cpt-cf-serverless-sdk-core-component-handler` — async trait implementation
* `cpt-cf-serverless-sdk-core-component-workflow` — async supertrait implementation
