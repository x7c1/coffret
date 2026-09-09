---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "secret-bearing inventory" docs/spec/device-key-custody/README.md && grep -q "secret-bearing inventory" docs/concepts/master-key/README.md && grep -q "span of a keyed operation" docs/spec/device-key-custody/README.md && grep -q "^- lock (" docs/concepts/master-key/README.md && grep -q "^- \*\*DK-10\.\*\*" docs/spec/device-key-custody/README.md && grep -q "DK-10" docs/concepts/recovery-code/README.md && grep -q "DK-10" backend/crates/apps/coffret-shell/src/recovery_code.rs && grep -q "DK-10" backend/crates/apps/coffret-cli/tests/setup.rs && grep -q "spent by the one unlock" docs/concepts/passphrase/README.md && ! grep -q "It never fires in the middle of an operation" backend/crates/apps/coffret-server/src/lock/lock_when_idle.rs && grep -q "defers rather than interrupts" backend/crates/apps/coffret-server/src/lock/lock_when_idle.rs && grep -q "device-local Library name" backend/crates/apps/coffret-device/src/testing/mod.rs && grep -q "device-local Library name" backend/crates/apps/coffret-device/tests/minio/mod.rs && grep -q "device-local Library name" backend/crates/apps/coffret-device/src/server_key.rs && ! grep -q "Library name" docs/concepts/container/entry/README.md && grep -q "carries the spec: prefix before the ID" docs/spec/README.md && ! grep -rqE "\((CP|KL|CK|OC|RV|EP|PK|MR|DK|FM|KD|EL|SA|LA)-[0-9]+" docs/concepts && ! grep -rq "(spec: " docs/spec'
assignee: null
branch: task/0909-1507-carry-key-custody-past-the-master-key
created_at: 2026-09-09T15:07:52Z
updated_at: 2026-09-09T19:48:27Z
---

# docs(spec): carry key custody past the Master Key and name how a secret is entered

## Overview

Device Key Custody (`docs/spec/device-key-custody/README.md`) states DK-7 over
the Master Key alone, while twenty code sites cite `(spec: DK-7)` for values
that are not the Master Key — `Passphrase`, `ContainerKey`, `PurposeKey`,
`RecoveryCode`, the Key Envelope plaintext, the stored-form plaintext, the token
cache's purpose key, `ControlKeys`, `LibraryKeys`. The complete list is written
down only in a Rust module doc
(`backend/crates/domain/coffret-model/src/master_key.rs`) and asserted in one
place (`backend/crates/domain/coffret-usecase/src/zeroization.rs`,
`every_secret_bearing_type_zeroizes_on_drop` and
`no_secret_bearing_type_is_clone`); the term "secret-bearing inventory" appears
in eleven code files and nowhere under `docs/`. DK-4 says inactivity locks the
device and never says what activity is, although
`backend/crates/apps/coffret-server/src/state.rs`, `lock/key_handle.rs`, and
`lock/idle.rs` settle it precisely and `tests/routes.rs` verifies it. The way a
Passphrase and a Recovery Code are entered on a device — a non-echoing prompt,
or one explicitly selected bounded line of standard input, never a command-line
argument — is stated in `docs/concepts/recovery-code/README.md` with no rule ID,
although `backend/crates/apps/coffret-shell/src/recovery_code.rs` and
`backend/crates/apps/coffret-cli/tests/setup.rs` verify it. Device Key Custody
runs DK-1 … DK-9, so DK-10 is free.

### Register

In `docs/spec/device-key-custody/README.md`:

- Two sub-items under DK-7, in the style of KD-9 and KD-10:
  - The claim covers not only the Master Key but everything a device holds that
    carries it or is derived from it: the Passphrase that unlocked it, the key
    that Passphrase derives (KD-5), the purpose keys (KD-3), the Container Keys
    those unwrap (KD-2), the Recovery Code form the key is written out as
    (KD-11), and the grouped key sets a run works under. Together these are the
    **secret-bearing inventory**, and a new type that comes to hold secret bytes
    joins it.
  - Inventory membership is the testable half of this rule: every type on the
    list overwrites its bytes when it is dropped and none of them is copyable,
    which one place asserts over the whole list rather than each type asserting
    its own. The absence claim above stays prose; this half is *(Form: test)*.
- A sub-item under DK-4:
  - Activity is the span of a keyed operation and not the moment a request
    arrived: the hold a piece of work takes on the unlocked Master Key marks the
    device as wanted when it is taken and again when it is let go, and the
    stretch between them counts as well — so work that outlasts the interval
    defers the lock rather than meeting it. A request that needs no key is not
    activity, since an open window asking what a device is doing is not a
    person at the keyboard.
- A new rule after DK-9:
  - **DK-10.** A secret a device is given to hold — the Passphrase that protects
    its stored Master Key, and the Recovery Code that carries the Master Key
    onto it — is taken from a non-echoing prompt, or, where a script selects
    that explicitly, from one bounded line of standard input. Neither is ever
    taken from a command-line argument, so neither becomes part of the
    process's argument list or of a shell history, and no refusal repeats what
    was entered. *(Form: test)*
    - Where both are given on standard input, the Recovery Code is the first
      line and this device's Passphrase the next, and the reader of the first
      takes exactly one line so the second is still there for the reader of the
      Passphrase. Redirected input is never consumed as though the script had
      selected it: an unattended run says so.

Cite DK-10 from the tests that verify it: the module doc and the four test
comments of `coffret-shell/src/recovery_code.rs`, and the
`join_help_names_only_secret_input_and_states_the_two_line_order` and
`active_round_trip_scripts_keep_the_recovery_code_out_of_join_arguments` cases
in `coffret-cli/tests/setup.rs`. Cite the DK-4 sub-item from the four idle-lock
cases in `coffret-server/tests/routes.rs`, and the DK-7 test half from
`zeroization.rs`.

In `docs/spec/README.md`, at the end of the "Rule IDs" section, write down the
citation convention the two documentation trees already follow (192 bare
citations under `docs/spec/`, 152 prefixed ones under `docs/concepts/`, and
none the other way): inside this register a rule cites a sibling rule bare, as
`KD-4` in parentheses; everywhere else a citation carries the spec: prefix
before the ID, so a reader of a concept document or a doc comment can see at a
glance that the token resolves here. Either spelling resolves, since an ID is a
unique token; the convention is about where the reader is standing. Write the
paragraph without a literal prefixed citation, so the register itself stays
free of one. The Rust and TypeScript trees mix both spellings; normalizing them
is not part of this change.

### Concept documents

- `docs/concepts/master-key/README.md`: add `lock` to the Collocations, as
  `- lock (the Master Key on a device, explicitly or after an idle interval)`;
  add a Domain Rule that an unlocked Master Key is locked again either because
  somebody asked or because the configured idle interval passed with no keyed
  work running, and a lock leaves nothing of it or of the keys derived from it
  in the process (spec: DK-3, DK-4, DK-7); and a Domain Rule that everything
  that carries the Master Key or is derived from it lives in a type that
  overwrites its bytes when it is dropped and that cannot be copied — the
  **secret-bearing inventory** — a closed list rather than a habit, because a
  guarantee about what is left in memory is only as good as the list it was
  checked against (spec: DK-7).
- `docs/concepts/passphrase/README.md`: a Domain Rule, placed second, that a
  Passphrase is spent by the one unlock that needs it and is not kept
  afterwards — a command reads it, unlocks, and exits; a server reads it once
  as it starts and thereafter holds only what the unlock produced, so a lock
  leaves nothing of the Passphrase or of those keys behind (spec: DK-1, DK-7,
  DK-9, DK-10) — with a sub-bullet that there is no route back through a
  browser: a locked server is unlocked by starting it again, because a
  Passphrase typed into a page would be a Passphrase carried through one
  (spec: DK-2). Verify the wording against `coffret-shell/src/passphrase.rs`
  and `coffret-server/src/state.rs`.
- `docs/concepts/recovery-code/README.md`: append `(spec: DK-10)` to the
  "Entering a code does not record it." rule.
- `docs/concepts/container/entry/README.md`: "Library name" is used there for
  the name a file has inside the Library, which collides with the device-local
  Library name defined in `docs/concepts/library/README.md`. Say "the Entry
  Path the file had at the moment the Container was written" instead.

### Doc comments

- `backend/crates/apps/coffret-server/src/lock/lock_when_idle.rs`: the
  paragraph beginning "It never fires in the middle of an operation." overstates
  and its own last clause concedes the window between reading the clock and
  emptying the cell. Replace it: it defers rather than interrupts — work that is
  running is somebody being here for the whole of it, so a piece of work that
  outlasts the interval pushes this back and the wait starts afresh from the
  moment it finished; what the lock ends is the next thing to ask. The moment
  between reading the clock and emptying the cell is not fenced against a
  request arriving in it, and does not need to be: whoever took a handle first
  finishes on it, exactly as under the explicit lock, and nothing is torn in
  half (spec: DK-2).
- In `coffret-device`, the bare phrase "Library name" always means the
  device-local one; spell it out as "device-local Library name" in
  `src/testing/mod.rs`, `tests/minio/mod.rs`, `src/server_key.rs`,
  `src/mapping/tests.rs`, and `src/recovery_code/tests.rs`. The sentence in
  `src/join_library/tests.rs` about a folder named after a Library is about the
  Library ID and is already correct.

This is a documentation change. No behaviour changes, no test is added, removed
or altered in what it asserts, and no module moves.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] DK-7 carries the secret-bearing inventory past the Master Key, and the
      Master Key concept gives the inventory a name a reader can resolve.
- [x] DK-4 says what activity is, and the Master Key concept carries the `lock`
      verb.
- [x] DK-10 states how a secret is entered at a device, and is cited from the
      Recovery Code concept and from both suites that verify it.
- [x] The Passphrase concept states the Passphrase's in-memory lifetime.
- [x] The idle-lock doc no longer claims it never fires mid-operation and says
      what it does instead.
- [x] `Library name` means one thing: the device crate's cases say
      device-local, and the Entry concept no longer uses the phrase for a
      file's name.
- [x] The register states the citation convention, and the concept and spec
      trees each hold only the spelling that convention gives them.
- [x] Existing backend, frontend, and interoperability checks continue to pass.

## Out of scope

No behaviour change of any kind: the idle lock, the secret readers, and the
zeroization assertions keep their current code. No new rule ID for the
inventory — DK-7 keeps its ID and its twenty existing citations stay correct.
No mass rewrite of rule-ID citations in the Rust or TypeScript trees. No new
test, and no change to what an existing test asserts. Loopback access and the
storage grant are a separate change.
