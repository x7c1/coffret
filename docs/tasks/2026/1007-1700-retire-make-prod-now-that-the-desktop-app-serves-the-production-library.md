---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make shell-lint && ! grep -qE "^prod:" Makefile && ! git grep -n "make prod" -- ":!docs/tasks" && ! git grep -n "production checkout" -- ":!docs/tasks" && grep -q "desktop app" docs/guides/environments.md'
assignee: null
branch: task/1007-1700-retire-make-prod-now-that-the-desktop-app-serves-the-production-library
created_at: 2026-10-07T07:35:24Z
updated_at: 2026-10-07T07:48:22Z
---

# build: retire `make prod` now that the desktop app serves the production Library

## Overview

The production Library has been served by `make prod` from a separate
production checkout kept on `main`, whose `local.mk` names the default state
directory (see `docs/guides/environments.md`). The installed desktop app
(`make desktop`, identifier `io.github.x7c1.coffret`) now does that job: it
serves the Libraries under the default state directory, takes the Passphrase in
its own window, unlocks in place after the idle lock, and runs beside the
development app. A person has verified it end to end on Linux. The production
checkout, its `local.mk` and `make prod` are no longer needed, and keeping them
leaves two ways to serve the same Library.

**1. Remove `make prod`.** Delete the `prod` target from the `Makefile` and the
`prod` mode from `scripts/dev.sh`, with everything only it used (the
fast-forward to `origin/main`, the explorer build plus `vite preview` on port
4173, and the dev/prod "kind" bookkeeping, if nothing else needs it). `make dev`
and `make down` keep working as they do. The refusal `scripts/dev.sh` prints
when something else answers at the port, and every comment in the `Makefile`
and the scripts, stop naming `make prod` or a production checkout. Keep the
`local.mk` includes: a machine-wide `~/.config/coffret/local.mk` (for
`LIBRARY := books`, say) is still useful, but no `local.mk` should name the
default state directory any more.

**2. Rewrite `docs/guides/environments.md` for the desktop app.** It should
read, in this order:
- **Two state directories:** the default one, where the production Library
  lives, and the development one every `make` target uses (unchanged in
  substance).
- **The production Library:** the installed desktop app serves it; install
  and update it with `make desktop` from a checkout on `main`, or from a
  GitHub Release (link `docs/guides/install/README.md`).
- **The development Library:** `make dev`, or the development app from
  `make desktop-dev`, both under the development state directory, never at
  the same time for one Library.
- **Production CLI operations:** the commands the desktop app does not offer
  (`init`, `join`, `authorize`, `map` with a prefix). A person runs them as
  `make cli COFFRET_STATE_DIR=<the default state directory> ARGS="…"` from a
  clean checkout of `main`, with the desktop app quit first. A command-line
  variable overrides the `Makefile`'s `?=`; say so, and say why `main`: a
  branch may have moved the Index schema.
- **The table of targets:** drop the production-checkout column.
- **The desktop app section:** call it "the desktop app" throughout, not
  "the desktop shell". The crate's type names are out of scope.
- **The rule for agents:** drop the bullet about `make prod` and the
  production checkout. Keep the rest, and add that an agent never runs
  `make cli` with `COFFRET_STATE_DIR` pointing at the default state
  directory.

**3. Other docs.** `README.md` and any other guide that sends a reader to
`make prod` or a production checkout points at the desktop app instead.

Out of scope: renaming the `Shell` type in `coffret-desktop`, the release
workflow, the desktop app's behaviour.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] The `Makefile` has no `prod` target and `scripts/dev.sh` no `prod` mode; nothing outside `docs/tasks/` mentions `make prod` or a production checkout
- [x] `docs/guides/environments.md` describes the production Library as served by the desktop app, the production CLI operations with an explicit `COFFRET_STATE_DIR`, and the agents' rule without `make prod`
- [x] `make shell-lint` passes
- [x] `make check` passes

### Before merge (verified outside the check command)
- [x] `make dev` and `make down` for a development Library still start and stop the pair — an agent runs this with a scratch Library it creates under the development state directory, or reports why it cannot
