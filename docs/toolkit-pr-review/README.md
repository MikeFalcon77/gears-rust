# toolkit-pr-review

Everything the `/toolkit-pr-review` skill reads at review time. This is **not** the Constructor
Studio review flow: that one lives in `docs/pr-review/` and is driven by `/cf-gears-pr-review`.
The two are unrelated and share no files.

| Path | What it is | Read by |
|---|---|---|
| `agents/subject.md` | Shared working instructions for the six subject agents | every subject agent |
| `agents/architecture.md` | Prompt for the PR-level pass | one agent per review |
| `rules/*.md` | The six rule modules, 51 check IDs | one module per agent |
| `comment-style.md` | How a finding is worded. The only authority on comment voice | every agent |
| `review-conventions.md` | Severity, criterion markers, reporting discipline | every agent |
| `RULES.md` | Generated index of every rule. Not authoritative, never hand-edited | humans |

Review work is split **by subject, not by file**: one agent per rule module, each reading the whole
diff, plus the architecture module. Each now has draft and closure invocations. An agent holds one module and nothing else. Measured across five
historical PRs under the earlier single-pass workflow, that reproduced 58% of a careful human reviewer's findings against 44% for the
file-sharding design it replaced; what moved the number is how many rules one agent carries at once,
not how many files it is given. `tools/scripts/toolkit-pr-review/budget.py` carries the table.

The canonical orchestrator is `.claude/skills/toolkit-pr-review/SKILL.md`. Repo-local
Claude, Codex, Cursor, Devin and Windsurf entrypoints refer to it. Claude/Codex
review-agent registrations refer to the shared instructions here; each module runs
a draft invocation and a separate source closure invocation.

`tools/scripts/toolkit-pr-review/lint.py` checks that the rules, the orchestrators and
`Cargo.toml` agree, and
regenerates `RULES.md` with `--write-index`.

## Pinned evidence workflow (schema 5)

Canonical instructions: `.claude/skills/toolkit-pr-review/SKILL.md`. The Python harness
uses only the standard library, never calls an LLM, and separates prepare, draft,
source closure, collect, matching, format and controlled publish. See
`policies/pass-contract.md` for transport; `policies/provenance.json` pins upstream
versions/hashes and exact original policies under `policies/upstream/`. Adapted policy
files are authoritative locally. No external RAG service or model override is installed.
Schema 4 directories remain historical and cannot pass schema 5 gates. Two source
passes and collect/matching/format are included in prepare's planning estimate.
Run `python -B tools/scripts/toolkit-pr-review/lint.py` and
`python -B -m unittest discover -s tools/scripts/toolkit-pr-review/tests`.
Offline replay checks wiring and preservation, not model quality; no paid review or
GitHub comment publication is part of the port. No claim of equivalent quality across
models follows from the transport validators.
