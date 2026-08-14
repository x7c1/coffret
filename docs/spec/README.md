# Specification

This register is the normative home for coffret's behavioral rules. The
concept documents in [docs/concepts/](../concepts/) define what each term
means and what a user can rely on; this directory defines how the system must
behave to honor those meanings — the procedures, verifications, and
parameters needed to build coffret correctly, and the obligations that no
test can execute.

## Rule dispositions

Every rule carries an explicit disposition tag:

- `→ tests` — the rule can be verified by executing code. It is here only
  until the test suite exists: the full rule statement will migrate into a
  test comment, the test cases become samples of the rule, and the rule here
  is then deleted or replaced by a one-line pointer to its test. This part of
  the register is designed to shrink.
- `prose-only (reason)` — the rule can never be executed as a test, so this
  register is its permanent home: obligations against adversarial
  counterparties (what may never be inferred from what Storage shows),
  completeness conditions involving the external world, and requirements
  aimed at other implementations.
- A partially testable rule keeps one ID and notes which part is test-bound
  and which part is prose-only.

The completeness condition for this register: every rule is either referenced
by a future test or explicitly marked prose-only with a reason.

Vocabulary, mental models, and guarantee summaries stay in the concept
documents; decisions and their rationale stay in design records. This
register holds only the current normative statements.

## Rule IDs

Every rule is a discrete statement with a stable ID: a short per-mechanism
prefix plus a number (`CP-3` is commit-protocol rule 3). IDs are permanent —
tests will reference them — so they are never renumbered or reused; a
migrated or withdrawn rule retires its ID.

Where a rule spans mechanisms, it lives in exactly one spec file; other files
reference its ID.

## Mechanisms

| Mechanism | Prefix | Covers |
| --- | --- | --- |
| [Commit Protocol](commit-protocol/) | `CP` | Journal head and commit slot, Keyring selection at commit, epoch activation fencing |
| [Keyring Lifecycle](keyring-lifecycle/) | `KL` | valid replica, complete set, committed, degraded, repair |
| [Checkpoint and Prune](checkpoint-and-prune/) | `CK` | Index Snapshot contents, prune eligibility and gating |
| [Orphan Cleanup](orphan-cleanup/) | `OC` | provenance-gated cleanup of uncommitted candidates |
| [Recovery](recovery/) | `RV` | restore inputs, salvage mode, bootstrap key derivation |
| [Entry Path](entry-path/) | `EP` | canonical form, comparison, collision, commit-time uniqueness |
| [Pack Construction](pack-construction/) | `PK` | freeze eligibility, segmentation, deletion, read-modify-replace |
| [Master Key Rotation](master-key-rotation/) | `MR` | epoch activation and rotation completion |
