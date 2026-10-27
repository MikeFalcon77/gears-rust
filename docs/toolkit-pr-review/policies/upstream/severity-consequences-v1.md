# Severity policy: consequences_v1

Assign severity to the consequences of this particular finding. Rule severity
lines and bracketed levels are historical guides, not required values, floors
or ceilings. This policy applies to every subject, architecture and overlays.
It does not change which criteria to check or which defects to report.

- CRITICAL: a realistic scenario causes a critical security breach, data loss
  or corruption, or unavailability of a substantial function. Establish the
  path to that consequence and the affected boundary. Harm to one customer's
  data or security can be critical; widespread impact is not required.
- HIGH: establish a substantial correctness, operability or contract risk in
  realistic use. An architecture recommendation or missing test alone is not
  enough. Describe the trigger, consequence and affected context.
- MEDIUM: an evidenced local defect, coverage gap or maintenance risk without
  established HIGH consequences. Do not discard it merely because it is MEDIUM.
- LOW: a small evidenced shortcoming. Cosmetic or speculative claims remain
  excluded; do not turn them into LOW findings to retain them.

Analysis assigns severity. For HIGH and CRITICAL, `impact` is mandatory and
must describe a realistic trigger, substantial consequence and affected
context. `evidence` connects the claim to concrete code. Text presence is not
proof: do not invent consequences to satisfy the output contract. If the issue
is established but serious consequences are not, choose the justified lower
level. Merge, matching and formatting must not recalibrate severity.

For RUST-TEST-001 assess the risk of the specifically uncovered behavior.
Missing a property test, negative example or assertion does not alone imply
HIGH. Missing coverage does not prove that production behavior is defective.
Explain the unprotected contract and why its realistic failure would matter.
Do not demand tests for a trivial refactor or rename without meaningful risk.
Apply all criteria with the same depth; lowering severity must not erase an
otherwise supported finding. Never upgrade a coverage claim into a proven
production bug without independent code evidence.
