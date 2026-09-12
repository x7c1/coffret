---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [concept-alignment, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_flushed_file.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/inspecting.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/state/inspecting.rs && grep -q "One scratch of" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_flushed_file.rs && ! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/destinations_conformance/round_trip.rs backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs && grep -q "creating a scratch in the folder must succeed" backend/crates/domain/coffret-usecase/src/destinations_conformance/round_trip.rs && grep -q "and the scratch would not go either" backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs && ! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/fetch_conformance && grep -q "and the scratch the run made is gone" backend/crates/domain/coffret-usecase/src/fetch_conformance/partial.rs && grep -q "scratch-and-rename" backend/crates/domain/coffret-usecase/src/fetch_conformance/fixtures/files.rs && ! grep -rqi "temporary" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "the scratch the cleanup could not remove is still there" backend/crates/domain/coffret-usecase/tests/place_faults.rs'
assignee: null
branch: task/0912-1420-say-scratch-in-the-usecase-test-facing-code
created_at: 2026-09-12T14:21:20Z
updated_at: 2026-09-12T17:58:00Z
---

# docs(backend): say scratch in the conformance suites and the fake filesystem, not temporary file

## Overview

The register owns the word. EP-11 defines it:

> A **scratch** is the file a local writer fills before the rename that
> publishes it. It is written inside a mapped folder, which is also a folder a
> scan walks, so coffret reserves a local filename prefix for it. Every local
> writer publishing by rename into one takes its scratch names from that
> prefix, and a scan passes over every local name carrying it instead of
> reporting it as a file to back up (EP-1, EP-8). A fetch gives its scratches
> no other kind of name, and neither does an upload the browser drops into a
> mapped folder, which is written and renamed for the same reason.

The Library concept says the same thing in its vocabulary list — "scratch
(bytes a local writer puts under the reserved prefix before the rename that
publishes them — a fetched Entry's, or a file taken into a mapped folder from
outside the Library)" — and in its Domain Rules: "A local writer writes its
**scratch** — the file it fills before the rename that publishes it — inside a
mapped folder ... A fetch is one such writer, and so is an upload the browser
drops into a mapped folder" (spec: EP-11). The plural EP-11 uses is
*scratches*.

A fetch is therefore one of the two writers the rule covers, which is why the
prose below is not uniform: where a case is about a fetch, *a fetch's scratch*
is the right thing to say, and where it states the reservation itself or a
contract both writers are held to, the writer is not a fetch in particular.

`coffret-usecase`'s own source now says *scratch* throughout: the `scratch`
module, the fetch path, the ports it writes through, and the scan that steps
over its names. Its **test-facing** code did not come along. The contradiction
is loudest where a case both spells the identifier and describes it:
`src/destinations_conformance/round_trip.rs` declares
`const SCRATCH: &str = ".coffret-fetch-a-case.part";` under a doc comment that
calls it "The name a case's temporary file goes by", and one line below says
"Coffret's own reserved scratch prefix". `tests/place_faults.rs` names its
cases `a_scratch_that_cannot_be_created_…`,
`a_write_that_fails_discards_the_scratch_…`,
`a_flush_that_fails_discards_the_scratch_…` and asserts on the fetch path's
warning "could not remove one of its own scratches" — while every doc comment
and assertion message around those names still says *temporary file*.

A conformance suite is the executable statement of what a rule means, so the
word it uses is read as the word the rule uses. This change makes the suites,
the in-memory fake they run against, and the placement-fault integration test
say what the register says.

### The count

Measured on `feat/mapped-root-marker`. `coffret-usecase` holds **43**
occurrences of `temporary` in 19 files. Sorted by what they mean:

| Meaning | Occurrences | Files |
| --- | --- | --- |
| This crate's scratch — drift, must become `scratch` | 35 | 11 |
| A conformance backend's throwaway root directory | 8 | 8 |

This change takes all 35 of the first kind. There is no third category in this
crate: the words `tempfile`, `TempDir` and `tempdir` do not occur in it at all,
so the survivors are prose about a directory a *backend* arranges for itself,
not calls into a crate. **Out of scope** enumerates them one by one, which is
the list to trust. The word is right in every one of them, and none of them
changes.

Per file, the 35:

| File | Occurrences |
| --- | --- |
| `tests/place_faults.rs` | 14 |
| `src/destinations_conformance/round_trip.rs` | 6 |
| `src/destinations_conformance/removal.rs` | 4 |
| `src/fetch_conformance/fixtures/files.rs` | 2 |
| `src/fetch_conformance/integrity.rs` | 2 |
| `src/fetch_conformance/partial.rs` | 2 |
| `src/fetch_conformance/adversarial_store.rs` | 1 |
| `src/fetch_conformance/round_trip.rs` | 1 |
| `src/in_memory_fs/in_memory_flushed_file.rs` | 1 |
| `src/in_memory_fs/inspecting.rs` | 1 |
| `src/in_memory_fs/state/inspecting.rs` | 1 |

No occurrence is capitalised: every one of the 35 is lower-case `temporary`
mid-sentence or at the start of a doc comment's own sentence ("A temporary
file…"), and a case-insensitive search over these paths finds nothing a
case-sensitive one misses. The criteria below still search
case-insensitively — the absence of a capital today is a fact about today, not
a property the check should rely on.

### The substitutions

Three, applied throughout:

- `temporary file` → `scratch`
- `temporary files` → `scratches`
- `temporary one` → `scratch` (the elliptical form the assertion messages use:
  "no temporary one behind" becomes "no scratch behind")

Every site is given in full below, because these are doc comments and assertion
messages whose line breaks matter: several paragraphs re-wrap when the phrase
shortens, and two of them wrap the phrase itself across a line break. The
re-wrapped lines below keep the ~80-column fill the files already use.

## What to change

**`backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_flushed_file.rs`** (1)

- Lines 12–13 re-wrap:
  ``/// One temporary file of [`InMemoryFs`](super::InMemoryFs) whose bytes are on``
  / `/// the device, waiting for its final name.` become
  ``/// One scratch of [`InMemoryFs`](super::InMemoryFs) whose bytes are on the``
  / `/// device, waiting for its final name.`

**`src/in_memory_fs/inspecting.rs`** (1)

- Lines 39–41 re-wrap. `/// What a case counting a fetch's leftovers reads: a temporary file lands`
  / `/// beside its Entry's own destination, so the question is about the whole`
  / `/// subtree rather than about one folder (spec: EP-11).` become
  `/// What a case counting a fetch's leftovers reads: a scratch lands beside`
  / `/// its Entry's own destination, so the question is about the whole subtree`
  / `/// rather than about one folder (spec: EP-11).`
  (all three lines carry the four-space indent of the `impl` they sit in.)

**`src/in_memory_fs/state/inspecting.rs`** (1)

- Line 84: `/// What a case counting a fetch's leftovers reads: a temporary file lands in`
  → `/// What a case counting a fetch's leftovers reads: a scratch lands in`.
  The three lines under it are unchanged: the sentence continues "whichever
  folder of the destination tree its Entry belongs in", which already starts
  the next line, and the shortened line pulls nothing up.

**`src/destinations_conformance/round_trip.rs`** (6) — the file that declares
`SCRATCH`, so its prose is the one that most obviously has to agree with it.

- Line 8: `/// The name a case's temporary file goes by.` →
  `/// The name a case's scratch goes by.`
- Line 39: `.expect("creating a temporary file in the folder must succeed");`
  → `.expect("creating a scratch in the folder must succeed");`. This is the
  one message this change pins, because it is the create inside a folder the
  descent had to make first — the whole of a placement in one line.
- Line 73: `"the rename moved the temporary file rather than leaving a copy of it",`
  → `"the rename moved the scratch rather than leaving a copy of it",`.
- Line 105: `.expect("creating a temporary file must succeed");` →
  `.expect("creating a scratch must succeed");`
- Lines 120–121 collapse to one. `/// A temporary file whose name is already taken is refused, and named as a`
  / `/// creation.` become
  `/// A scratch whose name is already taken is refused, and named as a creation.`
  The function is already `a_scratch_name_that_is_taken_is_refused`.
- Line 123 says `/// The scratch names a fetch draws are unique (see` and
  becomes `/// The scratch names a local writer draws are unique (see`. This
  doc states the shared `Destinations` contract, and `Destination::create` has
  two callers — a fetch and an upload — so the narrow word is wrong here even
  though the noun beside it was already right. `EP-11` gives the reservation to
  every local writer, and `crate::scratch`'s own module doc calls the prefix
  "one prefix, shared by everything that writes into a folder a scan walks",
  which is what makes the uniqueness claim a property of the reservation rather
  than of a fetch. Nothing else in the paragraph changes, and the line is not
  re-wrapped: at 58 characters it already ends where a fill at this file's
  width would break it.
- Line 132: `b"a temporary file some other run left",` →
  `b"a scratch some other run left",`.

**`src/destinations_conformance/removal.rs`** (4)

- Line 4: `/// Removing a temporary file that is already gone is success (spec: OC-6,`
  → `/// Removing a scratch that is already gone is success (spec: OC-6,`. The
  line break before `EP-11).` on line 5 stays where it is, so the change is to
  the noun and nothing else.
- Line 12: `/// to report with "and the temporary file would not go either".` →
  `/// to report with "and the scratch would not go either".` The fetch path's
  own counter-example already reads "and the scratch would not go either", so
  this brings the rule's statement back into step with the code it describes.
  This is the second message this change pins.
- Line 27: `.expect("a temporary file that was never created is already disposed of");`
  → `.expect("a scratch that was never created is already disposed of");`
- Line 32: `.expect("creating a temporary file must succeed");` →
  `.expect("creating a scratch must succeed");`. The binding on line 29 is
  already `let scratch = ".coffret-fetch-a-case.part";`.

**`src/fetch_conformance/adversarial_store.rs`** (1)

- Line 78: `"and the temporary file the run may have made is gone",` →
  `"and the scratch the run may have made is gone",`.

**`src/fetch_conformance/fixtures/files.rs`** (2, plus one compound)

- Line 59: `/// How many of a fetch's temporary files a folder still holds (spec: EP-11).`
  → `/// How many of a fetch's scratches a folder still holds (spec: EP-11).`
  The function it documents is already `scratch_left`.
- Lines 61–64 re-wrap as one paragraph, because line 63's occurrence and line
  62's `temp-and-rename` both shorten:
  `/// inside a mapped folder is exactly what the temp-and-rename exists to keep out`
  / `/// of a reader's way. The whole subtree, because a temporary file lands in`
  / `/// whichever folder of it its Entry belongs in.` become
  `/// inside a mapped folder is exactly what the scratch-and-rename exists to keep`
  / `/// out of a reader's way. The whole subtree, because a scratch lands in`
  / `/// whichever folder of it its Entry belongs in.`
  `temp-and-rename` is the last abbreviation of the noun left in the
  repository — it occurs exactly once, here — and it names the technique this
  very paragraph is about, so it moves with the rest rather than being left
  behind for a later pass. This change pins `scratch-and-rename`; the single
  hyphenated token cannot be split by a re-wrap.

**`src/fetch_conformance/integrity.rs`** (2)

- Line 24: `/// target path, and no temporary one either (spec: EP-11).` →
  `/// target path, and no scratch either (spec: EP-11).` Line 23 above it ends
  "no file at the", which still reads into the shortened line unchanged.
- Line 66: `"and the temporary file the run may have made is gone",` →
  `"and the scratch the run may have made is gone",`.

**`src/fetch_conformance/partial.rs`** (2)

- Line 138: `"a placed file leaves no temporary one behind (spec: EP-11)",` →
  `"a placed file leaves no scratch behind (spec: EP-11)",`.
- Line 223: `"and the temporary file the run made is gone",` →
  `"and the scratch the run made is gone",`. This is the third message this
  change pins.

**`src/fetch_conformance/round_trip.rs`** (1)

- Line 116: `"a placed file leaves no temporary one behind (spec: EP-11)",` →
  `"a placed file leaves no scratch behind (spec: EP-11)",` — the same message
  as `partial.rs` line 138, and it stays the same as it.

**`tests/place_faults.rs`** (14) — the placement-fault cases, whose function
names already say *scratch* and whose prose does not.

- Lines 134–136 re-wrap: `/// A temporary file a failed run left included, which is most of what these`
  / `/// cases read it for: only the one that got as far as the rename has`
  / `/// *placed* anything (spec: EP-11).` become
  `/// A scratch a failed run left included, which is most of what these cases`
  / `/// read it for: only the one that got as far as the rename has *placed*`
  / `/// anything (spec: EP-11).`
  (four-space indent, as above.)
- Line 303: `"the folder holds neither a placed file nor a temporary one: {:?}",` →
  `"the folder holds neither a placed file nor a scratch: {:?}",`.
- Lines 556–557 collapse to one:
  `/// A temporary file that cannot be created stops the run before a byte is`
  / `/// written.` become
  `/// A scratch that cannot be created stops the run before a byte is written.`
- Line 571: `.expect_err("the disk refused the temporary file");` →
  `.expect_err("the disk refused the scratch");`
- Line 574: `"the run failed creating the temporary file, and says so",` →
  `"the run failed creating the scratch, and says so",`.
- Line 579: `"no temporary file was made, so the folder holds nothing (spec: EP-11)",`
  → `"no scratch was made, so the folder holds nothing (spec: EP-11)",`.
- Line 587: `/// A write that fails takes the temporary file with it.` →
  `/// A write that fails takes the scratch with it.`
- Line 609: `"the temporary file is gone and nothing stands at the final name",` →
  `"the scratch is gone and nothing stands at the final name",`.
- Lines 619–620 re-wrap: `/// may not be. So the temporary file goes and the final name stays empty, rather`
  / `/// than a rename publishing bytes a crash could still lose.` become
  `/// may not be. So the scratch goes and the final name stays empty, rather than a`
  / `/// rename publishing bytes a crash could still lose.`
  Lines 616–618 of the paragraph are untouched.
- Lines 647–648 re-wrap: `/// what the Library holds, so the temporary file goes and the final name stays`
  / `/// empty, exactly as an unflushed one does.` become
  `/// what the Library holds, so the scratch goes and the final name stays empty,`
  / `/// exactly as an unflushed one does.`
- Line 669: `/// A rename that fails takes the temporary file with it too.` →
  `/// A rename that fails takes the scratch with it too.`
- Line 690: `"neither the temporary file nor the final one is there",` →
  `"neither the scratch nor the final file is there",`. The elliptical `one`
  took `file` from `temporary file` as its head noun; with `scratch` as one
  word the only noun left for it to take is `scratch`, and there is no "final
  scratch". The case asserts that neither the scratch nor the file that would
  have stood at the final name is there, so the head noun has to be said.
- Lines 701–705 re-wrap as one paragraph. The phrase wraps across 701–702 and
  line 703 says "a scratch file", which is the noun plus the word the noun
  replaced:
  `/// Replacing the failure that made the cleanup necessary with "and the temporary`
  / `/// file would not go either" would lose the verdict a caller acts on. What is`
  / `/// left is a scratch file no run will come back for, which the scratch prefix`
  / `/// keeps a scan from reading as user data — so what is lost is tidiness rather`
  / `/// than correctness, and the record is the only account anybody has of it.`
  become
  `/// Replacing the failure that made the cleanup necessary with "and the scratch`
  / `/// would not go either" would lose the verdict a caller acts on. What is left`
  / `/// is a scratch no run will come back for, which the scratch prefix keeps a`
  / `/// scan from reading as user data — so what is lost is tidiness rather than`
  / `/// correctness, and the record is the only account anybody has of it.`
  The quoted counter-example now matches the fetch path's own wording, and
  "a scratch file" becomes "a scratch".
- Line 725: `"the temporary file the cleanup could not remove is still there: {left:?}",`
  → `"the scratch the cleanup could not remove is still there: {left:?}",`. This
  is the fourth and last message this change pins. The `warn!` assertion below
  it, on line 733, already reads "could not remove one of its own scratches"
  and does not change.

No behaviour changes, no signatures change, no identifier is renamed, and no
assertion changes what it asserts — only the words the messages and the doc
comments use.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] The fake filesystem says *scratch*, and its flushed-file port says so
      first:
      `! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_flushed_file.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/inspecting.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/state/inspecting.rs`
      and
      `grep -q "One scratch of" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_flushed_file.rs`.
      Named file by file rather than sweeping `src/in_memory_fs/`, so the
      criterion covers exactly the files this change touches and a later file
      in the module cannot fail a check this change did not cover. The pinned
      phrase is three short words at the start of the line, ahead of an
      unbreakable intra-doc link, so a re-wrap cannot split it.
- [x] The two destinations cases about a scratch say *scratch*, and the rule's
      counter-example matches the one the fetch path writes:
      `! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/destinations_conformance/round_trip.rs backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs`
      and
      `grep -q "creating a scratch in the folder must succeed" backend/crates/domain/coffret-usecase/src/destinations_conformance/round_trip.rs`
      and
      `grep -q "and the scratch would not go either" backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs`.
      The absence gate deliberately names these two files rather than the
      `src/destinations_conformance/` directory. Sweeping the directory would
      fail on `root_arrangement.rs` and `destinations_under_test.rs`, which
      describe the throwaway root directory a backend arranges for itself and
      are right to say *temporary* (see **Out of scope**). Both pinned strings
      are Rust string literals, which `rustfmt` keeps whole on one line.
- [x] The whole fetch conformance suite says *scratch*, and so does the name
      of the technique:
      `! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/fetch_conformance`
      and
      `grep -q "and the scratch the run made is gone" backend/crates/domain/coffret-usecase/src/fetch_conformance/partial.rs`
      and
      `grep -q "scratch-and-rename" backend/crates/domain/coffret-usecase/src/fetch_conformance/fixtures/files.rs`.
      Here the pathspec is the whole directory, because this suite has no
      legitimate temporary of its own: every one of its five files that says
      the word says it about a fetch's scratch, and the suite runs against the
      in-memory fake, so no throwaway directory ever enters it. The pathspec is
      `src/fetch_conformance` and does not reach `src/fetch`, which already
      says *scratch* throughout.
- [x] The placement-fault cases say *scratch*, including the one message an
      operator-facing failure prints:
      `! grep -rqi "temporary" backend/crates/domain/coffret-usecase/tests/place_faults.rs`
      and
      `grep -q "the scratch the cleanup could not remove is still there" backend/crates/domain/coffret-usecase/tests/place_faults.rs`.
      Scoped to this one file rather than `tests/`, because
      `tests/sync_conformance.rs` says "no temporary directory" about what
      `cargo test` does not need, which is not a scratch.

Every absence gate above is written `-rqi`, case-insensitively. None of the 35
occurrences is capitalised today, but a case-sensitive gate would let a
sentence-initial *Temporary* slip back in and still read as satisfied — the
criterion would then claim a module says *scratch* while the module said
otherwise.

No criterion counts occurrences. A gate that pinned a number would turn a
miscount in the survey above into an unfixable check; the absence gates hold
whatever the true count turns out to be.

## Out of scope

The occurrences enumerated below mean *a temporary directory* in the ordinary
sense, and none of them changes. They describe the throwaway root a conformance
backend arranges for itself so that a suite can plant and read back files
without going through the capability under test. A scratch is a file a local
writer fills inside a mapped folder under a reserved prefix; a suite's
throwaway root is neither, and renaming it would make the sentence false. The
list is exhaustive rather than counted, so that a miscount cannot make it
wrong.

- **`src/destinations_conformance/root_arrangement.rs`**, line 11 — "It is the
  backend's instead — real files under a temporary directory for the gateway,
  entries in a map for the fake". The sentence's whole point is the contrast
  between the two backends; the gateway's half is a directory, not a scratch.
  This is the occurrence that makes the destinations gate above name files
  rather than sweep the directory.
- **`src/destinations_conformance/destinations_under_test.rs`**, line 40 — "A
  backend whose root is a temporary directory hands the owner over here rather
  than leaking it", on the `holding` method that keeps a directory guard alive
  for the length of a case. The second reason that gate names files.
- **`src/mapped_roots_conformance/folder_arrangement.rs`** line 11,
  **`src/mapped_roots_conformance/mapped_roots_under_test.rs`** line 39,
  **`src/spool_conformance/spool_under_test.rs`** line 31,
  **`src/commit_conformance/commit_under_test.rs`** line 39,
  **`src/index_conformance/index_under_test.rs`** line 33 — the same two
  sentences, for the other suites. Outside this change's file set as well as
  outside its meaning.
- **`tests/sync_conformance.rs`**, line 12 — "`cargo test` needs no container,
  no account, and no temporary directory", the module doc explaining what the
  in-process suite does without. A list of things not needed, one of which is a
  directory.

Worth recording, because it changes how a future reader should read the word in
this crate: `coffret-usecase` does not depend on the `tempfile` crate at all —
the tokens `tempfile`, `TempDir` and `tempdir` occur nowhere in it. The
survivors are prose about what an implementation of a suite's arrangement trait
does, not calls into a crate. Elsewhere in the backend the word usually does
mean that crate, and those occurrences are right for their own reason.

Also unchanged, deliberately:

- **`tests/place_faults.rs` line 303's `{:?}`** and the other positional
  format arguments in these files. Naming them would be a readability change
  with no bearing on the vocabulary, and it would touch lines this change has
  no reason to touch.
- **The identifiers.** Every function, constant and field in these files that
  refers to a scratch is already named for one — `SCRATCH`, `scratch_left`,
  `a_scratch_name_that_is_taken_is_refused`,
  `a_write_that_fails_discards_the_scratch_and_fails_the_fetch`. There is
  nothing to rename, and this change renames nothing.

Left to follow-up changes, because one change across all of them would be
unreviewable:

- `backend/crates/gateway/coffret-local-fs/src/unix_destinations/` (3 files).
- `backend/crates/apps/coffret-device/` — `src/add/` (3 files),
  `src/entry_fetches/mod.rs` and `src/run_fetch.rs`.
- `backend/crates/apps/coffret-server/` — `src/envelope.rs`, `src/fill/run.rs`,
  `src/routes/upload/receive.rs` and `tests/routes.rs`. The last of these
  shares a file set with `tests/support/mod.rs`, which plants fixture content
  under a real temporary directory, so that change's gate will have to name
  files rather than sweep `tests/`.
