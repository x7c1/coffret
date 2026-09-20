---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'refuse_my_drive_among' backend/crates/gateway/google-drive-store/examples/app_folders.rs"
assignee: null
branch: task/0920-0610-refuse-root-in-the-trash-subcommand-too
created_at: 2026-09-20T06:10:00Z
updated_at: 2026-09-20T12:00:00Z
---

# test(drive): refuse `root` in the folder tool's trash subcommand too

## Overview

`backend/crates/gateway/google-drive-store/examples/app_folders.rs`
refuses `root` as the parent of `list`: it is an alias for the top of
My Drive rather than a folder id, and nothing this tool does belongs
there. The `trash` subcommand takes ids without that check, so
`app_folders trash root` sends Drive a request to trash the top of My
Drive. Nothing happens — a `drive.file` grant has no such permission,
and the script only ever passes ids it read out of a listing — but the
two subcommands are not symmetric, and a person running the tool by
hand gets a Drive refusal instead of the tool's own explanation.

Make `trash` refuse the same way, before any request goes out:

1. **`trash` checks every id against `MY_DRIVE` first**, in a helper
   (`refuse_my_drive_among`) that `list` can share the wording with, and
   exits 1 with the same explanation `list` gives — before authorizing,
   before the first request. All-or-nothing: one `root` among several ids
   refuses the whole call, so a typo does not trash the rest either.
2. **The doc comment** at the top of the example says both subcommands
   refuse `root`, in the sentence that currently says it of `list`.

The offline path is testable for real: `app_folders trash root` and
`app_folders trash <some id> root` must exit 1 with the explanation and
never reach the network (no `Logging this run to` line from a request,
and no token cache is needed for the refusal — check that the refusal
comes before `api()` is built, so it works with the environment unset).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `app_folders trash` refuses `root` among its ids before any request or authorization, through a `refuse_my_drive_among` helper shared with `list`'s wording
- [x] The example's doc comment says both subcommands refuse `root`

### Manual / on-hardware (verified by a human before merge)

- [ ] `cargo run --release -p google-drive-store --example app_folders -- trash root` exits 1 with the explanation and without needing `COFFRET_DRIVE_*` set

## Out of scope

- Anything in `scripts/drive-it-reset.sh` or the Makefile
