# Specification

This register is the normative home for coffret's behavioral rules. The
concept documents in [docs/concepts/](../concepts/) define what each term
means and what a user can rely on; this directory defines how the system must
behave to honor those meanings — the procedures, verifications, and
parameters needed to build coffret correctly, and the obligations whose only
possible expression is prose.

## Rule Forms

Every rule carries a **Form** tag saying what expresses it — the medium that
can state the rule, not a pending verification act:

- `Form: test` — a test checks the rule: what the rule claims is something a
  case can observe coffret doing.
- `Form: prose` — only prose can state the rule, with a brief parenthetical
  reason. Typical cases: obligations against adversarial counterparties,
  completion conditions involving the external world, and interop
  requirements aimed at other implementations.
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

Vocabulary, mental models, and guarantee summaries stay in the concept
documents; decisions and their rationale stay in design records. This
register holds only the current normative statements.

### Rules and tests

Every rule is `Form: prose`, or `Form: test` and named by at least one test
that samples it, or marked `unimplemented`. A test names a rule by a comment
on the case that opens with the rule's ID followed by a colon — `// KD-4: …`
or `/// KD-4: …` (conformance cases are doc-commented) — and a case sampling
several rules lists them, as `// KD-4, KD-5: …`. `//!` is only for the
definition of a rule moved into a test's module doc (see
[Rule IDs](#rule-ids)), and that module doc names the rule it defines.

One rule is usually sampled by several tests, across crates and Storage
backends, and by convention every test that samples a rule names it, so a
rule's samples can all be found from its ID. `make spec-rule-ids` requires at
least one.

A rule whose behavior is not yet implemented carries `unimplemented` at the
end of its Form tag, as `*(Form: test; unimplemented)*`; on a rule of mixed
Form the mark applies to the test-bound part. The change that implements the
behavior adds the test and drops the mark together. A rule only some of whose
clauses are not yet implemented carries no mark: it says inside its Form tag
which clause is not yet implemented, as
`*(Form: test; not yet implemented: …)*` — the way a rule of mixed Form says
which part is which — and a test names it for the part that is.

A rule coffret decided not to implement is not kept: it is removed, or
replaced by a rule stating that coffret does not do it. Either way its ID is
not reused.

`make spec-rule-ids` refuses a `Form: test` rule here that no test names and
carries no mark, and a rule marked `unimplemented` that a test names: the
behavior exists, so the mark was left behind.

## Rule IDs

Every rule is a discrete statement with a stable ID: a short per-mechanism
prefix plus a number (`CP-3` is commit-protocol rule 3). IDs are never
renumbered or reused; a gap in the numbering means a rule once lived there,
and git history holds what it said.

A rule is defined in exactly one place, by default this register. A
`Form: test` rule whose tests are all in one module may instead have its full
statement and its ID moved into that module's doc comment, as `//! SA-8: …`,
with its entry here deleted in the same commit, as SA-8, SA-9 and KD-12 are.
Leaving a rule in the register is not unfinished work. Documents cite IDs as
plain text; an ID is a unique token, so a citation resolves by searching the
repository, wherever the rule is defined.

Where a rule spans mechanisms, it lives in exactly one spec file; other files
reference its ID.

A citation is spelled one of two ways, and which one is right depends on where
the reader is standing. Inside this register a rule cites a sibling rule bare,
as (KD-4): everything around it is already a rule, so nothing needs to say
where the token lives. Everywhere else — a concept document, a doc comment, a
test comment — a citation carries the spec: prefix before the ID, so a reader
who is not standing in the register can see at a glance that the token
resolves here. Either spelling resolves; the convention decides what a reader
is told, not what a search can find.

In code a citation sits in parentheses at the end of the sentence or clause
it stands behind, as `(spec: KD-4)` or `(spec: EP-9, EP-10)`, except that a
test comment naming the rule its case samples opens with the ID itself, as
`// KD-4: …` (see [Rules and tests](#rules-and-tests)). A comment cites a rule
where what it says is that rule's guarantee or rests on it — the doc comment
of the type or function that honors the rule, or the comment of a test case
that samples it — and not merely because the code nearby touches the
mechanism.

## Mechanisms

| Mechanism | Prefix | Covers |
| --- | --- | --- |
| [Commit Protocol](commit-protocol/) | `CP` | Journal head and commit slot, Keyring selection at commit, epoch activation fencing, an uploaded Container checked against the digest its write was answered with (CP-17) |
| [Keyring Lifecycle](keyring-lifecycle/) | `KL` | valid replica, complete set, committed, degraded, repair |
| [Checkpoint and Prune](checkpoint-and-prune/) | `CK` | Index Snapshot contents, prune eligibility and gating, when a Snapshot is written and its reserved slot, bringing a stale Index forward, what a catch-up holds while it does, more than one process sharing one Index (CK-13) |
| [Orphan Cleanup](orphan-cleanup/) | `OC` | provenance-gated cleanup of suspected orphans, completion of a landed commit's interrupted bookkeeping, the settlement a freeze begins with (OC-9), the idempotence of removing the local files and rows a device wrote for itself |
| [Recovery](recovery/) | `RV` | restore inputs, salvage mode, bootstrap key derivation |
| [Entry Path](entry-path/) | `EP` | canonical form, comparison, collision, commit-time uniqueness, local-root mappings, scan scope and fetch placement, the identity a mapped root is checked against before anything is placed into it, the name reserved for a device's management area under a mapped root |
| [Pack Construction](pack-construction/) | `PK` | freeze eligibility, the folder scope and explicit selection of one freeze invocation, and the reservation a drop's choice of a Pack is kept in until a freeze decides it (PK-17), segmentation, the entry table fixed before content is written (PK-18), update, deletion, read-modify-replace, the parcel a Container is fetched by and what the provider observes of it (PK-16, PK-19 to PK-22), and the repack that regroups a folder's Packs in path order (PK-23) |
| [Master Key Rotation](master-key-rotation/) | `MR` | epoch activation and rotation completion |
| [Device Key Custody](device-key-custody/) | `DK` | locked and unlocked states on a device, idle locking, handling of the unlocked Master Key, how a secret is entered at a device |
| [Loopback Access](loopback-access/) | `LA` | loopback-only listening, the per-run server key and the file it is published in, the admission fences every request passes, the budgets one request carrying files in is taken within, the one server at a time that serves a Library, the running state of the work a server runs, and the reservations it takes up as it starts and unlocks (LA-12), the explorer host's fixed authority, its admission of a browser by a browser token, and the one-time launch ticket that token is exchanged for (LA-13 to LA-21), and the desktop app's Passphrase door and the token it admits a window by (LA-22 to LA-25), and stopping an add, a freeze or a fill before it commits (LA-34 to LA-37) |
| [Storage Authorization](storage-authorization/) | `SA` | the authorization flow and its PKCE and loopback redirect, the one permission asked for, the grant width verified before anything is cached, what later runs mint, a grant kept once per device and account and the account-cache key envelope each Library opens it through, and what a page polling a served Library is told of a reconnect that renews a grant (SA-10) |
| [Storage Object Format](format/) | `FM` | Container v1 layout and chunked AEAD framing, meta section, Padmé padding, control-object framing and names, control payload schemas, Key Envelope form, the Library ID and the app folder a Library's objects live in |
| [Key Derivation](key-derivation/) | `KD` | Master and Container Key generation, HKDF purpose keys and the info registry, Argon2id protection of the stored Master Key and its byte layout, the sealed OAuth token cache's byte layout, the account-cache key and its envelope's byte layout, the Recovery Code encoding of the Master Key and its epoch |
| [Event Logging](event-logging/) | `EL` | diagnostic privacy boundary, `Redacted` grammar and cause chains, permitted local and Storage evidence |
