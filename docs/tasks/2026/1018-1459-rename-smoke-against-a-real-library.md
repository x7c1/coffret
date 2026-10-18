---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1018-1459-rename-smoke-against-a-real-library
created_at: 2026-10-18T05:59:54Z
updated_at: 2026-10-18T06:14:08Z
---

# test: add a make target that smoke-tests rename against a real Library

## Overview

CI exercises rename end to end against MinIO (`coffret-cli/tests/round_trip.rs`), and the conformance suites cover detection in memory. What is still checked by hand is whether the current build renames correctly against a real Library on a person's own Storage, such as a development Library on Drive.

Done by hand, that check is several commands with a Passphrase typed each time:

1. sync
2. rename a folder with `coffret rename`
3. rename two files on disk
4. sync again
5. read the logs to confirm that no Container was uploaded

This change makes that check a single command.

### The target

`make rename-smoke LIBRARY=<name>` runs `scripts/rename-smoke.sh`. It uses the same `COFFRET_STATE_DIR` / `COFFRET_LOG_DIR` defaults as `make cli`, which are the development state dir. A person can point it at another state dir the same way.

The script does the following:

1. **Asks for the Passphrase once,** with `read -rs`, and passes it to every step on standard input (`--passphrase-stdin`). It never echoes the Passphrase, puts it in arguments, or writes it to a file. A Drive client secret is not needed, because the Library stores its client.
2. **Builds the CLI once** from the current checkout, the same build `make cli` makes, and calls the binary directly.
3. **Picks a mapped root.** It reads the Library's mappings (`coffret mappings --library <name>`) and uses the Library-root mapping. If there is none, it uses the first mapping, and fails with a clear message if there are no mappings. Under that root it creates a fresh folder `rename-smoke-<UTC timestamp>/` holding two small books. Each book is a folder of a few small generated images; any non-empty distinct files are fine, and no new tooling dependency is allowed. The run touches nothing outside that folder.
4. **Runs the steps,** each with `--json`, and fails at the first one that does not hold:
   - **sync.** It adds the generated files.
   - **`coffret rename`.** It renames the first book's folder to `<smoke folder>/renamed/<book>`.
     - The answer's `renamed` equals the book's page count.
     - The commit adds and removes no Container.
     - Every file now stands at its new local path, and nothing is left at the old one.
   - **A rename on disk.** It renames one page of the second book within its folder, and moves another page into a new sub-folder of it.
   - **sync.** It carries both on-disk renames as renames.
     - The answer's `renamed` is 2 and `added` is 0.
     - The commit adds and removes no Container.
5. **Checks "no Container"** by reading the run's log, whose path the `--json` answer gives: the `committed the batch` event must show `additions` 0 and `removals` 0. The script relies only on fields the answer and that log event already carry; if one is missing, add it in the smallest way.
6. **Prints a short summary** of each step's result on success, and says which check failed on failure. Either way it states that the smoke folder stays in the Library and can be deleted from the explorer.
7. **Uses safe shell.** Use `set -euo pipefail`, quote every path, and never `rm` anything outside the smoke folder. Do not delete the smoke folder either: it is in the Library. The script must pass `make shell-lint`.

### Docs

Document the target in the Makefile help comment, as the other targets do, and in `docs/guides/environments.md` next to the development Library. Say what it needs, what it leaves behind, and that it uploads a few small files.

### Out of scope

A CLI `delete` and any cleanup of the smoke folder, running this in CI, and the server and explorer paths.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/rename-smoke.sh` and `make rename-smoke` exist, pass `make shell-lint`, and are documented in the Makefile help and `docs/guides/environments.md`.
- [x] The script fails clearly on the error paths: no mapping, a wrong Passphrase, and a check that does not hold. It never echoes the Passphrase or passes it in arguments. It writes only inside its own smoke folder.
- [x] Run against a MinIO Library created by the existing test helpers, with the Passphrase supplied the same way as `read -rs` would, the script passes end to end. This is tested by a CLI integration test or a script test that runs where the MinIO round trip runs, and that skips without MinIO as the round trip does.
