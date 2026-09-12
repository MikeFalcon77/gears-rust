#!/usr/bin/env python3
"""Consistency checks for the toolkit-pr-review rule set.

The rules live in the sub-agent prompts under docs/pr-review/agents/, one file per
agent. Nothing else defines them, so the failure mode is not drift between two copies
(there is only one) but drift between a rule and the things that must agree with it:
the orchestrator's routing table, the workspace lint config, and the pinned toolchain.

Usage:
    python3 tools/pr-review-lint.py            # check, exit 1 on failure
    python3 tools/pr-review-lint.py --write-index   # also regenerate docs/pr-review/RULES.md
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
AGENT_DIR = ROOT / "docs/pr-review/agents"
SKILL = ROOT / ".claude/skills/toolkit-pr-review/SKILL.md"
DEVIN = ROOT / ".devin/workflows/toolkit-pr-review.md"
CONVENTIONS = ROOT / "docs/pr-review/review-conventions.md"
STYLE = ROOT / "docs/pr-review/comment-style.md"
CARGO = ROOT / "Cargo.toml"
TOOLCHAIN = ROOT / "rust-toolchain.toml"
INDEX = ROOT / "docs/pr-review/RULES.md"

RULE_ID = r"(?:RUST|TOOLKIT)-[A-Z]+-\d{3}|TEST-QUALITY-\d+"
SEVERITIES = ("CRITICAL", "HIGH", "MEDIUM", "LOW")

failures: list[str] = []
notes: list[str] = []


def fail(check: str, msg: str) -> None:
    failures.append(f"[{check}] {msg}")


def note(msg: str) -> None:
    notes.append(f"  note: {msg}")


def agent_files() -> list[Path]:
    return sorted(AGENT_DIR.glob("toolkit-pr-review-*.md"))


def agent_name(p: Path) -> str:
    return p.stem.replace("toolkit-pr-review-", "")


def parse_rules() -> dict[str, dict]:
    """rule id -> {agent, severity, title, line}. Also records severity-less headings."""
    rules: dict[str, dict] = {}
    for path in agent_files():
        who = agent_name(path)
        lines = path.read_text(encoding="utf-8").splitlines()
        for i, line in enumerate(lines):
            m = re.match(rf"^###\s+(?:\d+\.\s+)?({RULE_ID})\s*(?:[—-]\s*(.*))?$", line)
            if not m:
                continue
            rid, title = m.group(1), (m.group(2) or "").strip()
            sev = None
            for nxt in lines[i + 1 : i + 4]:
                sm = re.match(r"^\*\*Severity\*\*:\s*(\w+)", nxt)
                if sm:
                    sev = sm.group(1).upper()
                    break
            if rid in rules:
                fail("duplicate-rule",
                     f"{rid} defined in both {rules[rid]['agent']} and {who}; a rule must have exactly one owner")
                continue
            rules[rid] = {"agent": who, "severity": sev, "title": title, "line": i + 1, "path": path}
    return rules


def check_severity(rules: dict[str, dict]) -> None:
    for rid, r in sorted(rules.items()):
        if r["severity"] is None:
            fail("severity", f"{rid} ({r['agent']}) has no `**Severity**:` line")
        elif r["severity"] not in SEVERITIES:
            fail("severity", f"{rid} ({r['agent']}) has severity {r['severity']!r}, expected one of {'/'.join(SEVERITIES)}")


def check_routing(rules: dict[str, dict]) -> None:
    """Every rule must appear in SKILL.md Step 4b, and vice versa."""
    if not SKILL.exists():
        fail("routing", f"{SKILL} not found")
        return
    text = SKILL.read_text(encoding="utf-8")
    m = re.search(r"### Step 4b.*?(?=\n### Step 4c)", text, re.S)
    if not m:
        fail("routing", "could not locate Step 4b in SKILL.md")
        return
    block = m.group(0)
    listed = set(re.findall(RULE_ID, block))

    # Ranges like "TOOLKIT-CORE-001..003" and "TEST-QUALITY-1 through TEST-QUALITY-10"
    for fam, lo, hi in re.findall(r"((?:RUST|TOOLKIT)-[A-Z]+)-(\d{3})\.\.(\d{3})", block):
        listed.update(f"{fam}-{n:03d}" for n in range(int(lo), int(hi) + 1))
    for lo, hi in re.findall(r"TEST-QUALITY-(\d+)\s+through\s+TEST-QUALITY-(\d+)", block):
        listed.update(f"TEST-QUALITY-{n}" for n in range(int(lo), int(hi) + 1))

    defined = set(rules)
    for rid in sorted(defined - listed):
        fail("routing", f"{rid} is defined in agent '{rules[rid]['agent']}' but not listed in SKILL.md Step 4b")
    for rid in sorted(listed - defined):
        fail("routing", f"{rid} is routed in SKILL.md Step 4b but no agent defines it")


# Lints reached through a denied group rather than named in Cargo.toml. Verified by
# compiling a triggering snippet under `#![deny(clippy::pedantic)]` with clippy-driver
# and reading the "implied by" note. Extend this only with the same evidence.
GROUP_MEMBERS = {
    "cast_lossless": "pedantic",
    "ptr_as_ptr": "pedantic",
    "fn_params_excessive_bools": "pedantic",
    "must_use_candidate": "pedantic",
}


def denied_lints() -> tuple[set[str], set[str]]:
    """(individually denied lint names, denied group names) from Cargo.toml [workspace.lints.*]."""
    if not CARGO.exists():
        return set(), set()
    text = CARGO.read_text(encoding="utf-8")
    lints, groups = set(), set()
    for block in re.findall(r"^\[workspace\.lints\.[a-z]+\]\n(.*?)(?=^\[|\Z)", text, re.S | re.M):
        for line in block.splitlines():
            m = re.match(r'^\s*([a-z_]+)\s*=\s*(.+)$', line)
            if not m:
                continue
            name, val = m.group(1), m.group(2)
            if '"deny"' not in val and '"forbid"' not in val and "level = \"deny\"" not in val:
                continue
            (groups if name in {"all", "pedantic", "nursery", "cargo", "complexity",
                                "correctness", "perf", "style", "suspicious"} else lints).add(name)
    return lints, groups


def check_enforcement(rules: dict[str, dict]) -> None:
    """Every `Enforcement: clippy <lint> (deny)` must name a lint the build actually denies."""
    lints, groups = denied_lints()
    if not lints and not groups:
        fail("enforcement", "could not read any denied lints from Cargo.toml [workspace.lints]")
        return
    seen = 0
    for path in agent_files():
        who = agent_name(path)
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for m in re.finditer(r"`?Enforcement: (?:clippy|rustc) ([^`(]+?)\s*\((deny|forbid)\)", line):
                named = [x.strip().strip('`') for x in re.split(r"[,/]| and ", m.group(1)) if x.strip()]
                for lint in named:
                    seen += 1
                    if lint in lints or lint in groups:
                        continue
                    grp = GROUP_MEMBERS.get(lint)
                    if grp and grp in groups:
                        continue  # reached through a denied group; membership verified
                    if grp:
                        fail("enforcement",
                             f"{who}:{n} claims `{lint}` is denied via the `{grp}` group, "
                             f"but that group is not denied in Cargo.toml")
                    else:
                        # Unverified is a failure, not a note. A denied group being present is not
                        # evidence that this particular lint is in it, and a note nobody acts on
                        # lets a marker drift away from the build silently.
                        extra = (f" A denied group is present ({sorted(groups)}): if the lint really "
                                 f"comes from it, verify with clippy-driver and add it to GROUP_MEMBERS."
                                 if groups else "")
                        fail("enforcement",
                             f"{who}:{n} claims `{lint}` is denied, but Cargo.toml does not deny it "
                             f"and it is not a verified member of a denied group.{extra}")
    if seen == 0:
        fail("enforcement", "no Enforcement markers found in any agent prompt; expected several")


def check_versions() -> None:
    """Version markers must parse, and dead ones (above the pin) are reported."""
    pin = None
    if TOOLCHAIN.exists():
        m = re.search(r'channel\s*=\s*"([0-9.]+)"', TOOLCHAIN.read_text(encoding="utf-8"))
        if m:
            pin = tuple(int(x) for x in m.group(1).split("."))
    if pin is None:
        fail("versions", "could not read the toolchain pin from rust-toolchain.toml")
        return
    dead = 0
    for path in agent_files():
        who = agent_name(path)
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for kind, ver in re.findall(r"(?:Requires|needs)\s+(Rust|Clippy)\s*>=\s*([0-9.]+)", line):
                parts = tuple(int(x) for x in ver.split("."))
                if kind == "Rust" and parts > pin[: len(parts)]:
                    dead += 1
    if dead:
        note(f"{dead} criteria are gated above the pinned toolchain "
             f"{'.'.join(map(str, pin))} and cannot fire today")


def check_contract_and_walk() -> None:
    """Each agent must document the three text fields and the per-file walk."""
    for path in agent_files():
        who = agent_name(path)
        text = path.read_text(encoding="utf-8")
        for field in ("comment", "issue", "fix"):
            if f'`"{field}"`' not in text:
                fail("contract", f"{who} Output Contract does not document the `{field}` field")
        if "one at a time" not in text:
            fail("walk", f"{who} is missing the per-file walk instruction in Scope Rules")
        for shared in ("review-conventions.md", "comment-style.md"):
            if shared not in text:
                fail("shared", f"{who} does not point at {shared}")


def check_shared_files() -> None:
    for f in (CONVENTIONS, STYLE):
        if not f.exists():
            fail("shared", f"{f.relative_to(ROOT)} is missing")
    if DEVIN.exists():
        t = DEVIN.read_text(encoding="utf-8")
        if "review-conventions.md" not in t:
            fail("harness", "the Devin workflow does not mention review-conventions.md")


def write_index(rules: dict[str, dict]) -> None:
    order = {s: i for i, s in enumerate(SEVERITIES)}
    rows = sorted(rules.items(), key=lambda kv: (order.get(kv[1]["severity"], 9), kv[0]))
    out = [
        "# Rule index",
        "",
        "**Generated by `tools/pr-review-lint.py --write-index`. Do not edit by hand.**",
        "",
        "Not authoritative: each rule is defined in its owning agent prompt under",
        "`docs/pr-review/agents/`, and that definition is the one the review applies.",
        "",
        f"{len(rules)} rules across {len({r['agent'] for r in rules.values()})} agents.",
        "",
        "| Rule | Severity | Agent | Title |",
        "|---|---|---|---|",
    ]
    for rid, r in rows:
        out.append(f"| `{rid}` | {r['severity'] or '—'} | {r['agent']} | {r['title'] or ''} |")
    INDEX.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"  wrote {INDEX.relative_to(ROOT)} ({len(rules)} rules)")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--write-index", action="store_true", help="regenerate docs/pr-review/RULES.md")
    args = ap.parse_args()

    rules = parse_rules()
    if not rules:
        fail("parse", f"no rules found under {AGENT_DIR.relative_to(ROOT)}")

    check_severity(rules)
    check_routing(rules)
    check_enforcement(rules)
    check_versions()
    check_contract_and_walk()
    check_shared_files()

    by_agent: dict[str, int] = {}
    for r in rules.values():
        by_agent[r["agent"]] = by_agent.get(r["agent"], 0) + 1
    print(f"  {len(rules)} rules: " + ", ".join(f"{k}={v}" for k, v in sorted(by_agent.items())))

    if args.write_index:
        write_index(rules)

    for n in notes:
        print(n)
    if failures:
        print(f"\nFAILED ({len(failures)}):")
        for f in failures:
            print(f"  {f}")
        return 1
    print("\nOK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
