# Specification

This register is the normative home for coffret's behavioral rules. The
concept documents in [docs/concepts/](../concepts/) define what each term
means and what a user can rely on; this directory defines how the system must
behave to honor those meanings — the procedures, verifications, and
parameters needed to build coffret correctly, and the obligations whose only
possible expression is prose.

## Rule Forms

Every rule carries a **Form** tag stating its final form — the medium that
can express the rule, not a pending verification act:

- `Form: test` — the rule's final form is a test plus its comment. The prose
  here is an interim expression awaiting migration: once the test exists, the
  full statement and its ID move into the test comment, the test cases become
  samples of the rule, and the entry here is deleted — in the same commit, so
  the migration itself guarantees nothing is lost. Only `Form: test` entries
  shrink and vanish.
- `Form: prose` — this statement is already its final form; it remains here
  permanently as the normative reference, with a brief parenthetical reason.
  Typical cases: obligations against adversarial counterparties, completion
  conditions involving the external world, and interop requirements aimed at
  other implementations.
- A rule whose parts differ in Form keeps one ID and notes which part is
  test-bound and which part is prose, in Form terms, inside the rule.

To choose between them, ask what the rule claims. A rule about a transition
coffret's own interface can be observed making is test-bound. A rule claiming
the *absence* of something across an open world is not: no test observes that
no copy of a key remains anywhere in a process image, or that nothing ever
reached a filesystem where swap and crash dumps are written outside coffret's
own writes. Such a rule is prose, and its parenthetical says how it is
honored instead — by construction, by review — since a test cannot.

Form is an inline per-rule attribute and can be reclassified later; files are
carved by mechanism only, so rules of both Forms stay adjacent within their
mechanism's context.

The completeness condition for this register: every rule is either
`Form: prose`, or `Form: test` and eventually referenced by a test.

Vocabulary, mental models, and guarantee summaries stay in the concept
documents; decisions and their rationale stay in design records. This
register holds only the current normative statements.

## Rule IDs

Every rule is a discrete statement with a stable ID: a short per-mechanism
prefix plus a number (`CP-3` is commit-protocol rule 3). IDs are never
renumbered or reused; a gap in the numbering means a rule once lived there,
and git history holds what it said.

A rule and its ID move together: before migration this register is the ID's
authoritative home, and after migration the test comment is. Documents cite
IDs as plain text; an ID is a unique token, so a citation resolves by
searching the repository for whichever home currently holds it. The
`Form: test` entries still present here are exactly the rules not yet
migrated; migrating one deletes its entry and adds the owning test in the
same commit, so completeness needs no ledger.

Where a rule spans mechanisms, it lives in exactly one spec file; other files
reference its ID.

## Mechanisms

| Mechanism | Prefix | Covers |
| --- | --- | --- |
| [Commit Protocol](commit-protocol/) | `CP` | Journal head and commit slot, Keyring selection at commit, epoch activation fencing |
| [Keyring Lifecycle](keyring-lifecycle/) | `KL` | valid replica, complete set, committed, degraded, repair |
| [Checkpoint and Prune](checkpoint-and-prune/) | `CK` | Index Snapshot contents, prune eligibility and gating, Snapshot upload after commit and its reserved slot, bringing a stale Index forward |
| [Orphan Cleanup](orphan-cleanup/) | `OC` | provenance-gated cleanup of suspected orphans |
| [Recovery](recovery/) | `RV` | restore inputs, salvage mode, bootstrap key derivation |
| [Entry Path](entry-path/) | `EP` | canonical form, comparison, collision, commit-time uniqueness, local-root mappings and scan scope |
| [Pack Construction](pack-construction/) | `PK` | freeze eligibility, segmentation, update, deletion, read-modify-replace |
| [Master Key Rotation](master-key-rotation/) | `MR` | epoch activation and rotation completion |
| [Device Key Custody](device-key-custody/) | `DK` | locked and unlocked states on a device, explicit and idle locking, handling of the unlocked Master Key |
| [Storage Object Format](format/) | `FM` | Container v1 layout and chunked AEAD framing, meta section, Padmé padding, control-object framing and names, Key Envelope form |
| [Key Derivation](key-derivation/) | `KD` | Master and Container Key generation, HKDF purpose keys and the info registry, Argon2id protection of the stored Master Key and its byte layout, the sealed OAuth token cache's byte layout |
