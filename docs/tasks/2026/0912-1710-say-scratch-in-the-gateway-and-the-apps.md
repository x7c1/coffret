---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [concept-alignment, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqi "temporary" backend/crates/gateway/coffret-local-fs/src/unix_destinations && ! grep -rqi "scratch file" backend/crates/gateway/coffret-local-fs/src/unix_destinations && grep -q "scratch on this device" backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_scratch_file.rs && ! grep -rqi "temporary" backend/crates/apps/coffret-device/src/add/incoming_file.rs backend/crates/apps/coffret-device/src/add/mod.rs backend/crates/apps/coffret-device/src/add/receive_file.rs && grep -q "left a scratch behind" backend/crates/apps/coffret-device/src/add/incoming_file.rs && ! grep -rqi "half-written fetch" backend/crates/apps/coffret-device/src/add/tests.rs && grep -q "whichever local writer" backend/crates/apps/coffret-device/src/add/tests.rs && ! grep -rqi "temporary" backend/crates/apps/coffret-device/src/entry_fetches backend/crates/apps/coffret-device/src/run_fetch.rs && grep -q "its scratch into the destination directory" backend/crates/apps/coffret-device/src/run_fetch.rs && ! grep -rqi "temporary" backend/crates/apps/coffret-server/src/envelope.rs backend/crates/apps/coffret-server/src/fill/run.rs backend/crates/apps/coffret-server/src/routes/upload/receive.rs && grep -q "scratch name that is removed" backend/crates/apps/coffret-server/src/routes/upload/receive.rs && ! grep -rqi "temporary name" backend/crates/apps/coffret-server/tests/routes.rs && grep -q "scratch name the bytes" backend/crates/apps/coffret-server/tests/routes.rs'
assignee: null
branch: task/0912-1710-say-scratch-in-the-gateway-and-the-apps
created_at: 2026-09-12T17:10:14Z
updated_at: 2026-09-12T19:11:34Z
---

# docs(backend): say scratch in the gateway and the apps, not temporary file

## Overview

The register owns the word. EP-11, read on `feat/mapped-root-marker`, defines
it:

> A **scratch** is the file a local writer fills before the rename that
> publishes it. It is written inside a mapped folder, which is also a folder a
> scan walks, so coffret reserves a local filename prefix for it. Every local
> writer publishing by rename into one takes its scratch names from that
> prefix, and a scan passes over every local name carrying it instead of
> reporting it as a file to back up (EP-1, EP-8). A fetch gives its scratches
> no other kind of name, and neither does an upload the browser drops into a
> mapped folder, which is written and renamed for the same reason.

The Library concept says the same in its vocabulary list — "scratch (bytes a
local writer puts under the reserved prefix before the rename that publishes
them — a fetched Entry's, or a file taken into a mapped folder from outside
the Library)" — and in its Domain Rules: "A local writer writes its
**scratch** — the file it fills before the rename that publishes it — inside a
mapped folder, which is also a folder a scan walks … A fetch is one such
writer, and so is an upload the browser drops into a mapped folder"
(spec: EP-11). The plural EP-11 uses is *scratches*.

Two things follow, and both matter here. The word is *scratch*, not *temporary
file*. And the writer is **a local writer** — a fetch and an upload are the
two of them — so prose about a scratch must not be narrowed to a fetch.

`coffret-usecase` now says *scratch* throughout, source and test-facing code
alike. The crates below it and above it did not come along:

- `coffret-local-fs`'s `unix_destinations` is the implementation of the very
  ports `coffret-usecase` calls `ScratchFile` and `FlushedFile`. The types are
  `UnixScratchFile` and `UnixFlushedFile`, their fields are `scratch_name`, and
  each one's opening doc line calls the thing "One temporary file on this
  device's disk".
- `coffret-device`'s `add` module is the **upload** side of EP-11 — the second
  local writer. `IncomingFile` holds a `scratch_name`, hands back a
  `scratch_path()`, and draws its name from `scratch::incoming_name()`, while
  seven doc comments, two in-line comments and one operator-facing `warn!`
  around them say *temporary file*. Its own test module, whose case is
  `a_dropped_file_under_the_scratch_prefix_is_refused`, explains the prefix as
  what keeps "a half-written **fetch**" from becoming an Entry — narrowing to
  one writer a rule that covers both, in the file that tests the other one.
- `coffret-server` reaches the same reservation from three directions: the free
  space it measures beside the file being written, the fill that shares one
  placement between callers, and the upload route that takes one part in.

### The count

Measured on `feat/mapped-root-marker` with `git grep`. Across the file groups
this change covers, `temporary` occurs **25** times in **13** files. Sorted by
what it means:

| Meaning | Occurrences | Files |
| --- | --- | --- |
| A scratch — drift, must become `scratch` | 22 | 12 |
| A real throwaway directory a test arranges | 3 | 2 |

Per file, the 22:

| File | Occurrences |
| --- | --- |
| `backend/crates/apps/coffret-device/src/add/incoming_file.rs` | 10 |
| `backend/crates/apps/coffret-device/src/add/mod.rs` | 2 |
| `backend/crates/gateway/coffret-local-fs/src/unix_destinations/mod.rs` | 1 |
| `backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_flushed_file.rs` | 1 |
| `backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_scratch_file.rs` | 1 |
| `backend/crates/apps/coffret-device/src/add/receive_file.rs` | 1 |
| `backend/crates/apps/coffret-device/src/entry_fetches/mod.rs` | 1 |
| `backend/crates/apps/coffret-device/src/run_fetch.rs` | 1 |
| `backend/crates/apps/coffret-server/src/envelope.rs` | 1 |
| `backend/crates/apps/coffret-server/src/fill/run.rs` | 1 |
| `backend/crates/apps/coffret-server/src/routes/upload/receive.rs` | 1 |
| `backend/crates/apps/coffret-server/tests/routes.rs` | 1 |

The three survivors are enumerated under **Out of scope**. Two of them sit in
`src/add/tests.rs` and one in `tests/routes.rs` — inside this change's own file
groups, which is what decides the shape of every gate below.

None of the 22 is capitalised: every one is lower-case `temporary` mid-sentence
or opening a doc comment's own sentence. The gates still search
case-insensitively, because the absence of a capital today is a fact about
today and not a property a check should lean on.

### Three drift sites the word *temporary* cannot find

Searching for `temporary` is not enough, and three sites prove it. All three
are changed here.

- `unix_destinations/open_folder.rs` line 22 and
  `unix_destinations/unix_destination.rs` line 15 say **"scratch file"** — the
  register's noun with the word it replaced stuck back on the end. A scratch
  *is* a file; "scratch file" reads as a kind of file rather than as the thing
  EP-11 names. Both lines are in the directory this change sweeps, and neither
  contains `temporary`, so only reading them finds them.
- `src/add/tests.rs` lines 370–371 say the prefix keeps "a half-written
  **fetch**" from becoming an Entry. That is EP-11 narrowed to one of its two
  writers, stated in the test module for the other one. It is the same drift a
  merged change has already repaired once in `coffret-usecase`, and this is
  where the remaining instance in the upload path sits.

A sweep for `temp`, `tmp`, `.part`, `scratch file` and `scratch-file` over
every path below turns up nothing else: the only other hits are the three
legitimate survivors, the `.coffret-fetch-*.part` fixture names (which are the
reserved prefix spelled out, and correct as they stand), and
`std::env::temp_dir()`.

### The substitutions

- `temporary file` → `scratch`
- `temporary name` → `scratch name`
- `scratch file` → `scratch`

Two sites take a rewrite rather than a substitution, because the mechanical
result would double the noun ("the scratch name is coffret's reserved scratch
prefix"). Both are given in full below.

The backend has no `rustfmt.toml`, so `wrap_comments` is off and `cargo fmt`
neither re-wraps a doc comment nor splits a string literal. Every re-wrap below
is therefore this change's own, done to keep the ~80-column fill the files
already use; every pinned string is a single line in the tree this change
produces.

## What to change

### `backend/crates/gateway/coffret-local-fs/src/unix_destinations/`

**`mod.rs`** (1)

- Line 15: `//! Every write is then made *relative to that handle* — the temporary file, the`
  → `//! Every write is then made *relative to that handle* — the scratch, the`.
  Line 16 already opens `//! rename that publishes it, the removal that cleans
  it up`, so the trailing `the` stays — it belongs to "the rename", and dropping
  it would leave "— the scratch, rename that publishes it" and break the
  three-item list. The substitution touches one line.
  `coffret-usecase/src/destination.rs:11` already reads "— the scratch, the
  rename that publishes it, the" after the first pass; this is the same sentence
  on the gateway side, and the same shape.

**`mod.rs`** (the narrowing to repair, 1) — the module doc's own summary line.

- Line 1: `//! Where a fetched Entry is placed on this device, and the walk that reaches`
  → `//! Where a local writer puts a file on this device, and the walk that reaches`.
  This module implements `ScratchFile`, `FlushedFile` and `Destination`, and an
  upload the browser drops goes through the same descent and the same
  `create` / `publish` as a fetch does — `Destination::create` is reached from
  `coffret-usecase/src/fetch/placement.rs` and from
  `coffret-device/src/add/incoming_file.rs`, which holds a
  `Box<dyn Destination>`. EP-11 gives publishing by rename to a local writer
  and names a fetch and a browser-dropped upload as the two, so the summary
  line was narrower than the module it introduces. Lines 2 onward are already
  writer-neutral and do not move. One line for one line, so the line numbers
  below it are unchanged.

**`unix_destination.rs`** (the narrowing to repair, 2 lines) — the comment
inside `Destination::create` explaining the mode it asks for.

- Lines 32-33: `// for: the file becomes the Entry's on the rename, and a fetch does not`
  / `// decide the permissions of a person's own folder.` become
  `// for: the file becomes the person's own on the rename, and a local`
  / `// writer does not decide the permissions of a person's own folder.`
  What an upload places is not an Entry at the rename — the next sync makes it
  one — and this is the implementation both writers call, so neither half of
  the old sentence held generally. "The person's own" is the register's own
  phrase for it (`coffret-device/src/add/added_at.rs:36`,
  `add/receive_file.rs:44`). It is also what the mode is chosen for: if the
  file becomes the person's own at the rename, its permissions are their
  umask's business rather than a writer's. Two lines for two lines; line 62's
  citation does not move.

**`unix_flushed_file.rs`** (1) — the doc line above `struct UnixFlushedFile`,
whose own field is `scratch_name`.

- Lines 11–12 re-wrap.
  `/// One temporary file on this device's disk whose bytes are on the device,`
  / `/// waiting for its final name.` become
  `/// One scratch on this device's disk whose bytes are on the device, waiting`
  / `/// for its final name.`

**`unix_scratch_file.rs`** (1) — the doc line above `struct UnixScratchFile`.

- Line 10: `/// One temporary file on this device's disk, open for writing.` →
  `/// One scratch on this device's disk, open for writing.` This is the one
  line this change pins in the directory: the type's name and its first
  sentence saying the same word is the whole point of the pass.

**`open_folder.rs`** (the doubling, 1)

- Line 22: `/// the destination the caller keeps, the scratch file it opened, and the flushed`
  → `/// the destination the caller keeps, the scratch it opened, and the flushed`.
  The next line continues "file that renames it are three handles on one open
  folder", which is unchanged and still reads correctly.

**`unix_destination.rs`** (the doubling, 1)

- Line 15: `/// scratch file it opens and the flushed file that renames that scratch file are`
  → `/// scratch it opens and the flushed file that renames that scratch are`. Both
  occurrences on the line go; "the flushed file" stays, because that one is the
  `UnixFlushedFile` port's own name.

### `backend/crates/apps/coffret-device/src/add/`

**`incoming_file.rs`** (10) — the upload writer itself.

- Line 17: `/// no-silent-selection posture EP-4 sets. Every call below — the temporary file,`
  → `/// no-silent-selection posture EP-4 sets. Every call below — the scratch,`.
  Line 18 already opens `/// the rename, the removal — is then made against that
  open folder rather than`, so nothing else moves.
- Line 23: `/// temporary file beside their destination as they arrive, the file is flushed,`
  → `/// scratch beside their destination as they arrive, the file is flushed,`.
  Line 22 ends "The bytes go into a", which reads into the shortened line
  unchanged.
- Line 34 is a **rewrite**, not a substitution. `/// The temporary name carries coffret's reserved scratch prefix`
  would become "The scratch name carries coffret's reserved scratch prefix",
  doubling the noun. It becomes
  `/// Its scratch name comes from coffret's reserved prefix`, which is EP-11's
  own construction ("takes its scratch names from that prefix"). Lines 35–36 —
  ``/// ([`scratch`](coffret_usecase::scratch)), so a transfer that stops halfway``
  / `/// leaves a name the scan steps over rather than one it reads as user data.`
  — are unchanged.
- Lines 38–42 re-wrap, because the phrase wraps across the 38/39 break.
  ``/// A value that is dropped without [`keep`](Self::keep) removes its temporary``
  / `/// file: an abandoned upload is a request that went away, and the folder should`
  / `/// not accumulate what it left. That is what makes dropping it the right thing`
  / `/// to do with one — there is no state to unwind and nothing to report, because`
  / `/// nothing has become visible.` become
  ``/// A value that is dropped without [`keep`](Self::keep) removes its scratch:``
  / `/// an abandoned upload is a request that went away, and the folder should not`
  / `/// accumulate what it left. That is what makes dropping it the right thing to`
  / `/// do with one — there is no state to unwind and nothing to report, because`
  / `/// nothing has become visible.`
- Line 76: `/// The open temporary file, until it is flushed.` →
  `/// The open scratch, until it is flushed.` (four-space indent; it documents
  the `file: Option<Box<dyn ScratchFile>>` field.)
- Line 82: `/// Opens a temporary file in the folder a descent arrived at.` →
  `/// Opens a scratch in the folder a descent arrived at.` (four-space indent.)
- Line 147: `// The temporary file is what the failure leaves behind, and the drop`
  → `// The scratch is what the failure leaves behind, and the drop`
  (twelve-space indent, inside the `match` arm.)
- Line 158: `// The temporary file is still there, and the drop guard is what` →
  `// The scratch is still there, and the drop guard is what` (twelve-space
  indent.)
- Line 178: `/// Where the temporary file being written stands, for an error to name — or`
  → `/// Where the scratch being written stands, for an error to name — or`. Line
  179 continues "for a caller with something to ask the filesystem about the
  volume" and does not move. The method below is already `scratch_path`.
- Line 223, inside the drop guard's `warn!`:
  `"an upload that did not finish left a temporary file behind",` →
  `"an upload that did not finish left a scratch behind",`. This is the one
  occurrence an operator reads, so it is the one this change pins for the
  module. Its sibling on line 214 already reads "an upload that did not finish
  left nothing behind" and does not change.

**`mod.rs`** (2) — the module doc's "Whole or absent" section.

- Line 32: `//! [`IncomingFile`] writes to a temporary name inside the destination directory`
  → ``//! [`IncomingFile`] writes to a scratch name inside the destination directory``.
- Line 34 is the second **rewrite**. `//! path is either nothing or the whole file (spec: EP-11). The temporary name is`
  would become "The scratch name is coffret's reserved scratch prefix", doubling
  the noun across the line break into line 35. It becomes
  `//! path is either nothing or the whole file (spec: EP-11). The name comes from`,
  so that line 35 — `//! coffret's reserved scratch prefix, which the scan already steps over` —
  reads on from it unchanged.

**`receive_file.rs`** (1)

- Line 56: `/// temporary file could not be created.` →
  `/// scratch could not be created.` (four-space indent.) Line 55 ends
  "`Local` where the folders above the file could not be made, or the", which
  reads into the shortened line unchanged. The doc comment two paragraphs above,
  on line 39, already reads "the scratch prefix a half-written file is called
  by".

**`tests.rs`** (0 occurrences of `temporary` to change; 1 narrowing to repair)

Both of this file's `temporary` occurrences are legitimate and stay — see
**Out of scope**. What changes is the doc comment on
`a_dropped_file_under_the_scratch_prefix_is_refused`, which explains a rule
that covers both local writers as though it covered only a fetch. Lines
370–373:

```
/// The other half of one reservation. The prefix is what a scan steps over so
/// that a half-written fetch never becomes an Entry (spec: EP-11), which makes a
/// file taken in under it exactly as invisible to the Library as one under the
/// management area — and just as silently so, were it accepted.
```

become:

```
/// The other half of one reservation. The prefix is what a scan steps over so
/// that a half-written scratch never becomes an Entry, whichever local writer
/// left one (spec: EP-11) — an upload into a mapped folder as much as a fetch.
/// That makes a file taken in under it exactly as invisible to the Library as
/// one under the management area — and just as silently so, were it accepted.
```

The case is about an *upload* being refused at a scratch name, so a doc comment
that credits the reservation to a fetch alone misreads the very rule the case
stands on. "whichever local writer" is the phrase this change pins here: three
words, no article and no number, and short enough that a later re-fill of the
paragraph cannot split it.

### `backend/crates/apps/coffret-device/`

**`src/entry_fetches/mod.rs`** (1)

- Line 11: `//! temporary file, and rename onto one path — the second one over a file the`
  → `//! scratch, and rename onto one path — the second one over a file the`. Line
  10 ends "range-read the same extent of the same Container, write a", which
  reads into the shortened line unchanged.

**`src/run_fetch.rs`** (1)

- Line 22: `/// its temporary file into the destination directory, because the rename` →
  `/// its scratch into the destination directory, because the rename`
  (four-space indent.) Line 21 ends "a fetch commits nothing, and it writes".
  This is the pinned line for the pair: "its scratch into the destination
  directory" is the whole of why there is no spool, and it sits well inside one
  line.

### `backend/crates/apps/coffret-server/`

**`src/envelope.rs`** (1)

- Line 114: `/// The path is the temporary file the bytes are already going to, so the answer`
  → `/// The path is the scratch the bytes are already going to, so the answer`.
  Lines 115–116 continue "is about the volume they will land on rather than
  about wherever a folder name might have resolved to" and do not move.

**`src/fill/run.rs`** (1)

- Line 19: `/// is placed once rather than once per caller: one temporary file inside the`
  → `/// is placed once rather than once per caller: one scratch inside the`. Line
  20 already reads "mapped folder, and one rename into place (spec: EP-11)".

**`src/routes/upload/receive.rs`** (1)

- Line 20: `/// because the bytes are going to a temporary name that is removed when the`
  → `/// because the bytes are going to a scratch name that is removed when the`.
  Line 21 continues "incoming file is dropped (spec: EP-11)". This is the pinned
  line for the server's source files; line 35 of the same file already says "a
  scratch name that could not be created", so the two now agree, and the pinned
  fragment "scratch name that is removed" tells them apart.

**`tests/routes.rs`** (1 of its 2 occurrences)

- Line 1170: `// no scratch either — the temporary name the bytes were going to goes with the`
  → `// no scratch either — the scratch name the bytes were going to goes with the`.
  The comment already says *scratch* in its first clause, so the second clause
  saying *temporary name* about the same thing is the contradiction. Line 303's
  occurrence is a real throwaway directory and stays — see **Out of scope**,
  which is why this file gets a narrow gate rather than a sweep.

No behaviour changes, no signatures change, no identifier is renamed, and no
assertion changes what it asserts — only the words the doc comments, the
in-line comments and the one `warn!` message use.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] The destinations gateway says *scratch*, without the word it replaced and
      without doubling it:
      `! grep -rqi "temporary" backend/crates/gateway/coffret-local-fs/src/unix_destinations`
      and
      `! grep -rqi "scratch file" backend/crates/gateway/coffret-local-fs/src/unix_destinations`
      and
      `grep -q "scratch on this device" backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_scratch_file.rs`.
      The pathspec is the whole directory, which is safe here because this
      directory has no legitimate temporary of its own: all eight of its files
      were read, the tokens `tempfile`, `TempDir`, `tempdir` and `temp_dir`
      occur in none of them, and every one of the three `temporary` occurrences
      is about a scratch. The second gate is the one the word *temporary* could
      never have found — "scratch file" is the register's noun with the replaced
      word stuck back on — and it cannot be tripped by `ScratchFile` or
      `scratch_name`, neither of which has a space in it.
- [x] The upload writer says *scratch*, including the message an operator reads
      when an abandoned upload could not be cleaned up:
      `! grep -rqi "temporary" backend/crates/apps/coffret-device/src/add/incoming_file.rs backend/crates/apps/coffret-device/src/add/mod.rs backend/crates/apps/coffret-device/src/add/receive_file.rs`
      and
      `grep -q "left a scratch behind" backend/crates/apps/coffret-device/src/add/incoming_file.rs`.
      The absence gate names these three files and deliberately does **not**
      sweep `src/add/`. Sweeping it would fail on `src/add/tests.rs`, which
      builds its fixture under a real `TempDir` and says *temporary directory*
      about it correctly (see **Out of scope**). The pinned string is a Rust
      string literal, which `cargo fmt` keeps whole on one line, and it is
      distinct from the sibling message "left nothing behind" on line 214, nine
      lines above it.
- [x] The upload's own test module states the reservation for both local
      writers rather than for a fetch alone:
      `! grep -rqi "half-written fetch" backend/crates/apps/coffret-device/src/add/tests.rs`
      and
      `grep -q "whichever local writer" backend/crates/apps/coffret-device/src/add/tests.rs`.
      This file's two `temporary` occurrences are legitimate, so there is no
      absence-of-*temporary* gate for it; the narrow absence gate states exactly
      the narrowing being repaired, and the presence gate states what replaces
      it.
- [x] The single-flight module and the fetch entry point say *scratch*, and the
      reason there is no spool is stated in the register's word:
      `! grep -rqi "temporary" backend/crates/apps/coffret-device/src/entry_fetches backend/crates/apps/coffret-device/src/run_fetch.rs`
      and
      `grep -q "its scratch into the destination directory" backend/crates/apps/coffret-device/src/run_fetch.rs`.
      `src/entry_fetches` is swept as a directory: both its files were read, and
      `gates.rs` says the word nowhere while `mod.rs`'s one occurrence is a
      scratch. The pathspec reaches neither `src/add/` nor `src/owner_only.rs`,
      each of which has legitimate temporaries of its own.
- [x] The server's source says *scratch* on all three sides of the reservation
      it touches:
      `! grep -rqi "temporary" backend/crates/apps/coffret-server/src/envelope.rs backend/crates/apps/coffret-server/src/fill/run.rs backend/crates/apps/coffret-server/src/routes/upload/receive.rs`
      and
      `grep -q "scratch name that is removed" backend/crates/apps/coffret-server/src/routes/upload/receive.rs`.
      Named file by file rather than sweeping `src/`, so the criterion covers
      exactly the files this change touches and a later file in the crate cannot
      fail a check this change did not cover.
- [x] The route test's account of what a stopped drop leaves says *scratch* in
      both of its clauses:
      `! grep -rqi "temporary name" backend/crates/apps/coffret-server/tests/routes.rs`
      and
      `grep -q "scratch name the bytes" backend/crates/apps/coffret-server/tests/routes.rs`.
      The absence gate is narrowed to the phrase *temporary name* rather than to
      the word *temporary*, because this file itself holds a legitimate
      survivor: line 303 arranges a real throwaway directory with
      `tempfile::tempdir()` and expects "a temporary outside folder". No
      absence-of-*temporary* gate is possible on this file at all — not because
      of what its sibling `tests/support/mod.rs` says, but because of what this
      file says on its own line 303. The pinned fragment is four words of a
      line comment, well inside one line.

Every absence gate above is written case-insensitively (`-rqi`). None of the 22
occurrences is capitalised today, but a case-sensitive gate would let a
sentence-initial *Temporary* back in and still read as satisfied — the
criterion would then claim a module says *scratch* while the module said
otherwise. That has happened in this vocabulary change before.

No criterion counts occurrences, and no pinned fragment carries an article that
could reasonably be spelled the other way or a number that a later edit could
make wrong. A gate that pinned a count would turn a miscount in the survey
above into an unfixable check; the absence gates hold whatever the true count
turns out to be.

## Out of scope

### The legitimate survivors inside these file groups

Three occurrences of `temporary` in the paths above mean *a temporary
directory* in the ordinary sense. Each was read rather than pattern-matched,
and none changes — renaming any of them would make the sentence false. A
scratch is the file a local writer fills inside a mapped folder under the
reserved prefix; a test's throwaway directory is neither.

- **`backend/crates/apps/coffret-device/src/add/tests.rs`**, line 47 — "Both
  live under one temporary directory that travels with the fixture, so a"
  (describing the fixture, whose `_held: TempDir` field on line 58 keeps the
  directory alive for the length of a case). A directory the test arranges for
  itself, outside any mapped folder.
- **`backend/crates/apps/coffret-device/src/add/tests.rs`**, line 62 —
  `TempDir::new().expect("a temporary directory must be available")`. The
  `tempfile` crate's own notion, named in its own words. These two are why the
  upload gate names three files instead of sweeping `src/add/`. (Line 87's
  `spool: std::env::temp_dir()` is the standard library's function and no
  prose at all.)
- **`backend/crates/apps/coffret-server/tests/routes.rs`**, line 303 —
  `tempfile::tempdir().expect("a temporary outside folder")`, the directory
  *outside* every mapped root that the planted-symlink case points at. This one
  is why `tests/routes.rs` gets a narrow *temporary name* gate: an
  absence-of-*temporary* gate on this file could never pass.

### Occurrences the two earlier passes already covered

`coffret-usecase`'s own occurrences — the `scratch` module, the fetch path, the
ports, the scan, the conformance suites, the in-memory fake and
`tests/place_faults.rs` — are already *scratch* on `feat/mapped-root-marker` or
in the change immediately ahead of this one. Nothing in that crate is touched
here, and no gate above reaches into it.

### The citation change

`backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs`
line 62 cites `(spec: OC-6, EP-11)` in a comment about swallowing an
already-absent removal. Moving that citation from **OC-6 to OC-8** is a
separately queued change with its own reasons, and it is not made here even
though the file is in this change's file set: the vocabulary and the citation
are two different claims, and mixing them would make either one harder to
review. No gate above mentions `OC-6` or `OC-8`, so this change neither
enforces nor blocks it.

### Everything else in the backend that says *temporary* correctly

Untouched, and right as it stands:

- **`coffret-device/src/owner_only.rs`** — the `temporary_neighbour` helper, the
  `.tmp` suffix it builds, and the prose around it. These are files the device
  keeps for itself outside any mapped folder; nothing scans them, so the
  reserved prefix does not apply and neither does the word.
- **`google-drive-store/src/oauth/token_cache/`** — the same atomic-replace
  helper for the OAuth token cache, for the same reason.
- **`coffret-server/tests/support/mod.rs`**, and the `tempfile` fixtures
  throughout `coffret-device`'s, `coffret-cli`'s, `coffret-interop`'s and the
  gateways' tests — throwaway directories and fixture files, not scratches.
- **`coffret-device/src/mapping/tests.rs`**, `src/browse/tests.rs`,
  `src/run_catch_up.rs`, `src/testing/mod.rs` and the gateway conformance
  harnesses — the same.

### The identifiers, and the fixture names

Nothing is renamed. Every type, field, method and case in these files that
refers to a scratch is already named for one — `UnixScratchFile`,
`UnixFlushedFile`, `scratch_name`, `scratch_path`, `scratch::incoming_name`,
`the_scratch_of_an_interrupted_upload_is_not_a_row`,
`a_dropped_file_under_the_scratch_prefix_is_refused`. The
`.coffret-fetch-*.part` names in the fixtures are the reserved prefix spelled
out literally, which is what they are for, and they do not change either.
