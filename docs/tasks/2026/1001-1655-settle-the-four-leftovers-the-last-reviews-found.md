---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && ! grep -qE "map_err\(.cause. TransportError::Body" backend/crates/gateway/google-drive-store/src/http/answer_body.rs && grep -qF "mktemp -d \"\$TOOLS" scripts/shell-lint.sh && ! grep -qE "mktemp -d\)" scripts/shell-lint.sh && ! grep -qE "bail!" backend/crates/apps/coffret-cli/src/init.rs backend/crates/apps/coffret-cli/src/join.rs backend/crates/apps/coffret-cli/src/drive_client.rs && ! grep -qE "set wrongly .* is ..other.." backend/crates/apps/coffret-cli/src/answer/mod.rs && grep -qE "CK-6" docs/concepts/journal/README.md docs/concepts/library/README.md'
assignee: null
branch: task/1001-1655-settle-the-four-leftovers-the-last-reviews-found
created_at: 2026-10-01T16:55:00Z
updated_at: 2026-10-01T17:39:58Z
---

# fix: classify a document's read timeout, stage lint tools beside their cache, type the CLI's flag and secret refusals, and write down the join invariant

## Overview

The reviews of the last four merges each left one small finding that the PR
did not take. None depends on another and none is worth a pipeline of its own,
so they land together; each is listed with its own fix and its own gate, and a
reviewer can read the four hunks apart.

**1. A read timeout while collecting a document is a timeout, not a broken
connection.** `collect_within()` in
`backend/crates/gateway/google-drive-store/src/http/answer_body.rs` drains an
answer that declared no length and maps every read error to
`TransportError::Body` (the connection broke). `buffer()` in
`http/reqwest_transport.rs` drains the other kind of answer and goes through
`classify_read`, which looks for reqwest's timeout under the `io::Error` and
answers `TransportError::Timeout` for it — so the same stall is told as two
different things depending on whether the answer declared a length. Move
`classify_read` to a place the `http` module shares (the `transport_error`
module, as a constructor on `TransportError`, or a module of its own) and use it
from both. Pin it with a test next to the existing
`a_small_answer_that_stops_between_bytes_is_a_timeout` in
`reqwest_transport_tests.rs`: an answer that declares no length (chunked), sends
some bytes and then stops is a `Timeout`. The default deadline (30 s) fires
before the read timeout today, so nothing changes for the shipped values; the
fix is so that a longer deadline does not change what a stall is called.

**2. `scripts/shell-lint.sh` stages a download on the filesystem it will live
on.** The shellcheck branch unpacks into `mktemp -d` (under `TMPDIR`) and then
`mv`s the binary into `.tmp/tools/`. The script's own contract (its comment
above the two `if [ ! -x … ]` blocks) is that a present binary is a verified
one. That holds only when `mv` is a rename: with `TMPDIR` on another filesystem
it is a copy, and a run interrupted in the copy leaves a partial, executable
file that the next run's `[ -x ]` accepts. Stage under the cache itself —
`mktemp -d "$TOOLS/.fetch.XXXXXX"` after `mkdir -p "$TOOLS"` — so the final
step is a rename on every machine, and clean the staging directory up on the
failure paths too (a `trap` is the usual shape). The shfmt branch downloads to
`$shfmt.download` beside its target already; leave it, or fold both into the
one staging pattern if that reads better.

**3. The CLI's `--json` names the two refusals that fall through to
`"other"`.** `Failure::of` in
`backend/crates/apps/coffret-cli/src/answer/failure.rs` classifies an error
chain by type, and anything it has no type for is kind `"other"`. Two refusals
the CLI itself raises are `anyhow::bail!` strings and so land there: the flags
that do not go together (`provider()` in `init.rs` and `join.rs`:
`--drive needs --parent and --client-id`, `--s3 needs --bucket`, …) and the
`COFFRET_DRIVE_CLIENT_SECRET` variable set wrongly (`client_secret()` in
`drive_client.rs`: empty, or not Unicode). Give the CLI a refusal type of its
own for these (an enum with one variant per refusal, `thiserror`-style, the
sentences kept as they are), raise it in place of the `bail!`s, and have
`classified` name its variants. Pick the kinds so that every sentence of the
`--json` section of `--help` (`answer/mod.rs`) stays true: the help says
`"usage"` comes `with command and log null`, and these refusals are met after
the command is known and the log started, so either name them apart from
`"usage"` or reword that sentence. Then rewrite the help's last sentence about
`"other"` — once these two are named, `"other"` is only what no layer this
crate knows raised — and pin each new kind with a golden test beside the
existing failure goldens in `answer/tests.rs`. `coffret-cli` follows the
repository's error-type rules: no `PartialEq` on the error, causes kept as
values, the kind derived from the variant and listed exhaustively the way
`device_kind` lists the device's.

**4. The concept documents state the invariant `join` relies on.**
`FoundOnStorage` (`backend/crates/apps/coffret-device/src/join_library/found_on_storage.rs`)
and `ControlObjectName::names_a_head_or_index_snapshot` decide a join by asking
whether *any* head or ordinary Index Snapshot is at the place, because `prune`
may delete every head (CK-4, CK-6) but never the Snapshot that applied the last
of them (CK-2), so a Library that has committed anything always holds at least
one of the two. That reasoning exists only in those two doc comments. Write it
down where a reader of the concepts would look: under `## Domain Rules` of
`docs/concepts/journal/README.md` (the invariant: prune never leaves a Library
that has committed with neither a head nor a Snapshot, with the CK rule ids),
and in the join rules of `docs/concepts/library/README.md` (the join asks for
any head or Snapshot, and a place holding neither is "nothing yet" — a Library
created and not yet synced, not a failure). Keep the vocabulary the two
documents already use (*head*, *Index Snapshot*, *prune*, *checkpoint*, *join*)
and cite the spec rules as the documents do (`spec: CK-2, CK-4, CK-6`);
`make spec-citations` checks the ids. Point the two doc comments at the concept
documents once the text is there, rather than repeating the argument.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `collect_within` classifies read errors through the same function
      `buffer` uses; `collect_within` no longer maps read errors to
      `TransportError::Body` itself (grep gate in `check_command`), and a test pins that a
      chunked answer that stops between bytes is a `Timeout`.
- [x] `shell-lint.sh` stages its downloads under `$TOOLS` and no longer calls
      bare `mktemp -d` (grep gates); `make shell-lint` still passes from a
      clean `.tmp/tools/`.
- [x] `init.rs`, `join.rs` and `drive_client.rs` raise no `anyhow::bail!`
      (grep gate); the flag-combination refusals and the client-secret
      refusals each have a kind other than `"other"`, pinned by golden tests.
- [x] The `--json` help no longer lists those two refusals under `"other"`
      (grep gate) and still describes every kind the CLI can answer.
- [x] `docs/concepts/journal/README.md` and `docs/concepts/library/README.md`
      state the head-or-Snapshot invariant and the join's "nothing yet" with
      spec citations (`CK-6` appears in at least one; `make spec-citations`
      passes).

### Manual / on-hardware (verified by a human before merge)

- [ ] CI is green on the PR.
