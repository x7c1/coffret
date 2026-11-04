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
branch: task/1104-2224-restart-the-local-library-when-make-local-runs-again
created_at: 2026-11-04T13:24:56Z
updated_at: 2026-11-04T13:49:02Z
---

# fix(local): restart the local Library when `make local` runs again

## Overview

`make local` (`scripts/local.sh up`) hands over to `scripts/dev.sh up`.
When the server, the build and the host are all running already, that
prints one line, "Already up for the Library …", and reuses them. It says
nothing about when they were started. The server locks the Library after
the idle interval (30 minutes by default), so a person who runs `make local`
again to get back to work gets that line, opens the explorer, and then meets
"The Library is locked" with no idea why.

Change `make local` so that a second run does not reuse what is running.
Where the Library is already seeded, `up` first does what `make local-down`
does: it stops the three processes and the MinIO container. Then it starts
them again as a first start would. All data under `.tmp/local/` is kept,
since this is a restart and not `make local-reset`. Say in the output that
the running processes are being stopped and started again.

- Only `make local` changes. `make dev` (`scripts/dev.sh up` called
  directly) keeps its "Already up" reuse; this task does not touch it.
- Keep the existing refusals in `up` (a container that is not ours, another
  MinIO port) working. Run the restart at a point where a refusal still
  stops before anything is stopped, wherever that is possible.
- When nothing is running, `make local` behaves as it does today. The
  half-seeded path (no `seeded` file) also behaves as it does today.
- Update what describes the behaviour: the header comment of
  `scripts/local.sh` ("`down` then `up` is the restart"), the
  `make local` / `make local-down` text in
  `docs/guides/environments.md` ("Later starts find the Library and go
  straight to the three processes", the target table), and the `Makefile`
  help comment for `local` if it says the same thing. `make local-down local`
  stays valid as a restart; it simply becomes the same as `make local`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/local.sh up`, where the Library is seeded, stops the running processes and the MinIO container before starting them again, and keeps `.tmp/local/` (read from the script)
- [x] `make dev`'s reuse of running processes in `scripts/dev.sh` is unchanged
- [x] `docs/guides/environments.md` and the comments in `scripts/local.sh` describe a second `make local` as a restart
- [x] `make check` passes (including `shell-lint`)

### Before merge

- [x] Run `make local` twice in a row with Docker available. The second run stops the three processes and MinIO and starts them again, prints a launch file, and the Library still holds the seeded books (no new seed). Then run `make local-down`.
