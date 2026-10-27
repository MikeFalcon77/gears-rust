# Review harness port validation

Target baseline: `0d73651247b7351521c8b65239cc77b5ed765818`.
Branch: `codex/review-harness-port`. Upstream policy implementation:
`modcrafter77/nonstop-reviewer@c6ac6f3fb2bfd35fcb27b6838e60a8835e47cdb5`.
Source and adapted-policy SHA256 values are in `policies/provenance.json`; criterion
hashes are compared with the target baseline, not inferred from a matching count.

## Implemented and checked

The canonical skill and five repo-local aliases use one workflow. Seven Claude and
seven Codex registrations share subject/architecture instructions. Models inherit
from the caller. All 52 rules, their ownership and 312 subject criteria remain;
RUST-ARCH-001's nine structural checks also remain unchanged. Toolchain/MSRV and
CI-enforced exclusions remain active. Version labels are historical guidance, not
mandatory severity. Constructor Studio and product Rust code are outside this port.

Schema 5 prepares immutable head/actual merge-base diffs, freezes instructions and
runner hashes, and reads additional Git objects without changing the checkout.
Each module has draft and closure states. Checked output, source bindings and failed
attempts are retained. Malformed JSON, lost candidates, wrong quotes, missing evidence,
unresolved chains and partial passes cannot become a clean empty review. Legacy
schema 4 directories are rejected without upgrading their contents.

Collect reconstructs direct groups from original claims. LOW groups are allowed;
representative identity, anchor and strongest original severity stay unchanged.
Originals reach formatter as duplicate_evidence. Runtime and coverage obligations
stay distinct, including independent same-line findings. Matching receives prior
comments only after discovery/closure and collect. Formatter returns prose only.
LOW stays in findings/report, outside publication payload. Publisher checks live
head, uses COMMENT, journals before POST and blocks unknown-outcome retries.

`python -B -m unittest discover -s tools/scripts/toolkit-pr-review/tests`: **90 passed**,
including the earlier 38 tests (the old severity-inheritance expectation was updated
to the approved consequence policy). Tests use temporary Git histories, source fixtures
and mocked GitHub transport; they make no live network or model calls.
`python -B tools/scripts/toolkit-pr-review/lint.py`: **OK**, 52 rules / 312 criteria.
Skill metadata and agent TOML registrations are validated separately. No Cargo build,
Clippy or reviewed-source test execution is claimed.

## Saved #5391 and manual meaning check

The saved paid six raw claims are replayed into four groups: two LOW presentation
groups, one separate LOW test gap and one separate historical MEDIUM regression-
protection comment. This is a wiring/preservation check, not a fresh severity verdict.

The async/design pair describes the same task-local loss at spawn. The saved final
wording retains the conditional OAuth2 token-fetch/cache-miss path and diagnostics-only
effect; it does not claim every streamed LLM request uses the same outbound client.
Its fix includes using captured/re-scoped context at actual spawn-to-completion sites,
not merely exposing an unused helper.

The security/design pair jointly presents inbound ID validation and egress opt-in
policy. Their remedies are complementary, not interchangeable. The saved comment
retains the external-IdP/token-cache-miss condition, distinguishes validation/regeneration
from forwarding control, and does not describe a correlation ID as a credential.
The sample 64-character cap remains an example, not a proved security threshold.

The gateway-installation regression test remains separate from the runtime context
defect; a missing assertion does not prove that runtime is currently broken. The optional
absence-header test remains LOW. Historic MEDIUM is retained for replay, not treated
as an unconditional ground truth for all invoking models or future reviews.

## Small source controls

Pinned synthetic base/head controls are archived with hashes:

- Lost context: the new background task reads task-local state without re-scoping;
  the base installed the value inside the task. The negative joined path captures and
  re-scopes explicitly. They must not be treated as equivalent failing paths.
- Conditional auxiliary request: the main tunnel path does not establish ordinary
  HTTP propagation. Token mode performs a separate egress preflight. The receipt test
  checks real source quotes for entry, spawn, configured dispatch and terminal send,
  and requires the main/auxiliary/configuration inventory.
- Security control: callback construction attaches an administration bearer secret
  to a user-specified destination; the public diagnostic-label control carries no
  authority. This synthetic fixture has no real external sender: do not claim an
  executed exploit or live disclosure. Mechanical checks preserve a separately
  evidenced serious-impact classification and reject observability-only HIGH.
- Optional test gap: the integration wrapper asserts status for absence while a
  lower-level header-absence assertion already exists. This supports a bounded test
  improvement, not an invented runtime failure or automatic MEDIUM.

These source and saved-comment checks do not prove semantic precision of a new model
response, equal quality across models, or improved defect recall. A new paid review,
GitHub publication, deployment, push and PR creation are intentionally not performed
in this stage. The next real invocation must still inspect every final comment's
conditions before publication.
