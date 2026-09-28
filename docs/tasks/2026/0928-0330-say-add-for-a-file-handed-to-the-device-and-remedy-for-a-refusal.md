---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 0
check_command: 'make check && ! grep -rqE "FileNotTakenIn|receive_file" --exclude-dir=target backend/crates/ && ! grep -rqF "not taken in" --exclude-dir=target backend/crates/ && ! grep -rqiE "taken into a mapped|took a file into a mapped|taking a file in(to)?\b|file (was )?taken in\b" --exclude-dir=target backend/crates/ docs/concepts/ && ! grep -rqiE "(can|Passphrase) resolves?\b|resolved by the Passphrase|gesture that resolves" --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist --exclude-dir=dist-types backend/crates/ frontend/packages/ && ! grep -rPzq --exclude-dir=target "can\s*\n\s*///\s*resolve\b" backend/crates/'
assignee: null
branch: task/0928-0330-say-add-for-a-file-handed-to-the-device-and-remedy-for-a-refusal-retry-1
created_at: 2026-09-28T03:30:00Z
updated_at: 2026-09-28T04:13:04Z
---

# refactor: say add for a file handed to the device, and remedy for what clears a refusal

## Overview

Two words are used in a sense the concept documents or the spec give to another word. A reader who trusts that the concept documents, the spec and the code name the same thing with the same word draws the wrong conclusion at each of them. Neither fix changes what any code path does: one renames a function, an error variant and the sentence that variant prints, the other is prose. A third item fixes a flaky test case the check met on the way.

1. **The device's act of accepting a file is *add*, not *receive* or *take in*.** The Library concept registers "add (a file to a mapped folder where no Entry of the Library stands — a browser's drop, or the person copying it in — for a later run to carry into the Library)" (`docs/concepts/library/README.md`, Collocations, around line 70), and the module that does it is already `coffret-device/src/add/` with `AddedFile`, `added_locally` and the log field `operation = "add_file"` (`add/incoming_file.rs:146,196,205`). The code and one concept line call the same act by two other verbs:
   - `OpenLibrary::receive_file` (`coffret-device/src/add/receive_file.rs:91`, and its module file) → `add_file` (module `add/add_file.rs`). Callers: `coffret-server/src/routes/upload/receive.rs:72`, `coffret-device/src/catalog_refusal_tests.rs:75`, `add/tests.rs`, and the intra-doc links in `add/incoming_file.rs:163`, `error/mod.rs:898`, `lib.rs:117`.
   - `Error::FileNotTakenIn` (`coffret-device/src/error/mod.rs:640`) → `FileNotAdded`, with its `Display` "the file was not taken in" (`error/display.rs:348`) → "the file was not added", its `redacted()` form `Device::FileNotTakenIn` (`error/redacted.rs:171-172`) → `Device::FileNotAdded`, and every match arm, doc link and test assertion that names it (`error/source.rs:76`, `error/from.rs:58`, `error/mod.rs` around 599, 621, 656-658, 697-698, 788, 868-870, 907, `error/tests.rs:156-174`, `add/tests.rs` around 125-229, `coffret-server/src/api_error/from_error.rs:32`, `coffret-server/src/api_error/tests.rs:105-111`).
   - The prose that says a file is "taken in" or "taken into a mapped folder" in that sense: the log message "took a file into a mapped folder" (`add/incoming_file.rs:148`), `add/tests.rs` (around lines 6, 177, 236, 371, 509, 642), `error/mod.rs` (the sentences around 621, 658 and 698), and the Library concept's scratch entry "a file taken into a mapped folder from outside the Library" (`docs/concepts/library/README.md`, around line 75) → "a file added to a mapped folder from outside the Library". Read each sentence and say *add* in the form the sentence needs.
2. **What clears a refusal is its *remedy*, not what *resolves* it.** The spec keeps *resolve* for path resolution (EP-8 and the EP rules around it in `docs/spec/entry-path/README.md`) and says a refusal is *remedied* (EP-14's sub-bullets, `docs/spec/entry-path/README.md` around lines 317-319); the code already follows that elsewhere (`coffret-device/src/finding.rs:193`, `coffret-server/src/api_error/from_error.rs:270`). Six comments still use *resolve* for clearing a refusal or a finding: `coffret-server/src/api_error/mod.rs:150-151` ("which no Passphrase resolves", "is resolved by the Passphrase"), `coffret-server/src/api_error/declines.rs:11-12` ("nothing about this device can / resolve", split across two lines), `coffret-server/src/api_error/tests.rs:409`, `coffret-usecase/src/sync_conformance/roots.rs:269` and `coffret-usecase/src/index.rs:173` ("the gesture that resolves"), and `frontend/packages/gateway/api/src/refusal.ts:72` ("which no Passphrase resolves"). Say *remedies* / *remedied* (or *clears* where the sentence is about a state going away).

3. **The server's answer-contract case races a background fill.** `the_answers_the_explorer_reads_are_the_ones_this_server_sends` (`coffret-server/tests/routes/contract.rs`, around line 112) asks for `albums/notes.txt` through `GET /api/file`, which arms a fill of the holding folder (`coffret-server/src/routes/file.rs`, around line 96), and then lists `albums` without waiting for that fill. Whether `café.jpg` and `cover.png` read `remote` or `present` in the written listing therefore depends on how far the fill got, and the case fails under load (it failed once inside a full `make check` and passed when run alone). Wait for the fill with `served.fill_idle()` before any listing is taken, so every row of `albums` the Library holds is `present`, and rewrite `frontend/packages/gateway/api/src/contract/answers.json` with `COFFRET_WRITE_CONTRACT=1`. The `remote` state stays covered by the packed listing (`books/page-001.png`); correct the case's comment that says `albums` shows both states, and make the explorer's `contract.test.ts` pass over the rewritten file. This is a test-only fix of a flaky case, not a wire change.

Guards — these keep their current words, because each is a different sense:

- The server's HTTP route module `coffret-server/src/routes/upload/receive.rs` and the spec's "an upload received from the" (`docs/spec/entry-path/README.md:233`) describe receiving an HTTP request body, not the device adding a file. They stay.
- *Take in* for bytes read into memory or records read into the catalog (for example `google-drive-store/src/answer_ceiling.rs`, `coffret-usecase/src/commit/catch_up.rs:449,502`, `coffret-usecase/src/index_conformance/refusals.rs`) is a different sense and stays. So does "this device already holds cannot take it in" about an account (`coffret-device/src/error/display.rs:170` and its tests).
- *Resolve* for path resolution stays everywhere.

No wire value changes: `FileNotAdded` maps to the fetch's verdict in `api_error/from_error.rs` as `FileNotTakenIn` did, and no JSON field or kind is renamed.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No `FileNotTakenIn` or `receive_file` remains under `backend/crates/` (grep gate)
- [x] No "not taken in" remains under `backend/crates/` (grep gate): the variant's sentence is "the file was not added"
- [x] No "taken into a mapped", "took a file into a mapped", "taking a file in" or "file taken in" remains under `backend/crates/` or `docs/concepts/` (grep gate)
- [x] No *resolve* in the sense of clearing a refusal remains at the listed sites under `backend/crates/` or `frontend/packages/` (grep gate on "can resolve", "Passphrase resolves", "resolved by the Passphrase", "gesture that resolves")
- [x] No doc comment says "can" / "resolve" split across two lines (the form `declines.rs:11-12` has today), checked by a multi-line grep gate
- [x] The answer-contract case waits for the background fill before it lists, so its written listings no longer depend on timing (`make check`)
- [x] `make check` passes

## Last subagent report

Check phase (attempt 1) exited 101 in `make check`: `contract::the_answers_the_explorer_reads_are_the_ones_this_server_sends` (`crates/apps/coffret-server/tests/routes/contract.rs:94`) reported that `frontend/packages/gateway/api/src/contract/answers.json` is not what the server sends. The case passed when rerun alone and in ten further runs, including under CPU load; the cause is the unwaited background fill described in item 3 above. Retrying from `work` with item 3 added.
