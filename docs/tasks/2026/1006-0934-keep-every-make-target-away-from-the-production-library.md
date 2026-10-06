---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: null
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -qE "^export COFFRET_STATE_DIR \?=" Makefile && grep -qE "^export COFFRET_LOG_DIR \?=" Makefile && grep -q "coffret-dev" Makefile && grep -qE "^cli:" Makefile && grep -qE "^prod:" Makefile && grep -qE "^@docs/guides/environments.md$" CLAUDE.md && grep -q "COFFRET_STATE_DIR" docs/guides/environments.md && ! grep -q "COFFRET_STATE_DIR" CLAUDE.md && ! grep -qE "coffret-dev" backend/crates/apps/coffret-device/src/library_dir.rs'
assignee: null
branch: task/1006-0934-keep-every-make-target-away-from-the-production-library
created_at: 2026-10-06T09:34:00Z
updated_at: 2026-10-06T11:37:08Z
---

# build: keep every make target away from the production Library, and start the production pair from a built explorer

## Overview

A device keeps the Library a person actually uses under the binary's default
state directory — `$XDG_STATE_HOME/coffret`, or `$HOME/.local/state/coffret`
(`backend/crates/apps/coffret-device/src/lib.rs`, "One directory per
Library"). Today that is also where every `make` target points: `make server`,
`make web`, `make dev` and the CLI all take the default unless
`COFFRET_STATE_DIR` is in the environment. So a checkout on any branch —
including one whose Index schema has moved, which has already refused an
existing Library once — is aimed at the production Library by doing nothing.
Only the scripts under `scripts/` (`e2e-it.sh`, `drive-round-trip-it.sh`,
`drive-index-layout-it.sh`) set a state directory of their own under `.tmp/`.

From now on the two are told apart by checkout: a development checkout (this
clone and any worktree of it) never reaches the default state directory unless
told to, and one checkout on the machine is the production one, whose
`local.mk` names the default directory explicitly and which is only ever on
`main`. The binary's own default does not change — it is still the place a
person's Library lives — so nothing under `backend/` is touched by this task.

**1. The Makefile defaults every target to a development state directory.**
Right after the two `-include` lines at the top of `Makefile`, export

```make
export COFFRET_STATE_DIR ?= $(or $(XDG_STATE_HOME),$(HOME)/.local/state)/coffret-dev
export COFFRET_LOG_DIR ?= $(COFFRET_STATE_DIR)/logs
```

(`$(or …)` rather than a shell `${…:-…}`, which the binary would receive
unexpanded). `?=` is what lets a checkout's `local.mk`, or the environment,
name the production directory instead; the `-include`s have to come first for
that. `make server`, `make web`, `make dev`, `make down` and the new `cli`
target then all carry the variables; the scripts that already set their own
(`e2e-it.sh`, the `drive-*` ones) keep theirs, since an assignment inside the
script wins. `scripts/dev.sh` already absolutises a relative
`COFFRET_STATE_DIR`, and the new default is absolute, so it needs no change
for this. Rewrite the Makefile comments that describe `COFFRET_STATE_DIR`
(above `LIBRARY`, `server`, `web`, `dev`) so they say what the default now is,
that the binary's default directory is the production one, and that it is
reached only when `local.mk` or the environment names it.

**2. `make cli ARGS="…"` runs the CLI with the checkout's variables.** A
`cli` target that builds `coffret-cli` in release and runs it with `$(ARGS)`,
so that `init`, `join`, `authorize` and the rest typed by hand land in the
same state directory the server uses, instead of the binary's default. Add it
to `make help`.

**3. `make prod` starts the production pair, and `make down` stops it.**
`make dev` starts the server and the explorer's *dev server*, which reads the
checkout's source as it changes. The production pair is the same server and a
*built* explorer: `pnpm --filter @coffret/web build`, served with
`vite preview` (whose proxy `frontend/packages/apps/web/vite.config.ts` already
configures, because the journeys serve the built explorer that way). `prod`
also pins the checkout to the latest `main` before building: it refuses when
the checkout is on another branch or the tree is not clean, and otherwise
fast-forwards to `origin/main` (`git fetch origin && git merge --ff-only
origin/main`), so that "the production build" always means "the head of
`main`". Put the behaviour beside `up` in `scripts/dev.sh` (a third command,
or a flag on `up`) rather than in a second script, so the pid files, the
`running()` check, the Passphrase handling and `down` are shared: `make down`
must stop a pair started by either `dev` or `prod` for that Library, and `up`
of either kind must report the other kind as already up rather than start a
second server. `vite preview` listens on 4173 by default and the dev server
on 5173, which is what lets a production pair and a development pair run at
once on one machine; the server's `PORT` is per checkout (the production
checkout sets its own in `local.mk`). `say_where` reads the address off the
log already; make sure it finds `vite preview`'s line too. Document `prod` in
the Makefile and in `make help`, and say in the `dev` comment that the
production checkout is a separate clone whose `local.mk` sets
`COFFRET_STATE_DIR`, `COFFRET_LOG_DIR` and `PORT`.

**4. `docs/guides/environments.md` says which checkout serves which Library,
and `CLAUDE.md` expands it.** A new document (the first under `docs/guides/`)
for a person or an agent running coffret from a checkout: the development
checkout and its `coffret-dev` state directory, the production checkout — a
separate clone kept on `main`, whose `local.mk` names the binary's default
state directory, `COFFRET_LOG_DIR` and `PORT` — and what `make dev`, `make
prod`, `make cli` and `make down` do in each. It also carries the rule for
agents: the binary's default state directory holds the Library a person uses
every day, an agent never reads it or points a binary at it, everything goes
through `make`, and a binary run by hand always has `COFFRET_STATE_DIR` set
explicitly. Name the variable so the rule is greppable. `CLAUDE.md` gets one
line, `@docs/guides/environments.md`, so the document is in every session's
context; the rule itself is not written into `CLAUDE.md`, and the Makefile
comments point at the document rather than repeat it at length. Like every
document here it is self-contained: no private repository, folder id, project
name or port of a particular machine.

Out of scope: the server serving the built explorer itself (so that a
production install needs no node), and a `make install`. Both belong to a
release, not to this split.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `Makefile` exports `COFFRET_STATE_DIR` with `?=` to a `coffret-dev` directory under `XDG_STATE_HOME` or `$HOME/.local/state`, and `COFFRET_LOG_DIR` under it, after the `-include` lines
- [x] `make cli` and `make prod` exist and appear in `make help`
- [x] `docs/guides/environments.md` exists, names `COFFRET_STATE_DIR` in the rule that keeps agents off the default state directory, and `CLAUDE.md` expands it with a line `@docs/guides/environments.md` instead of carrying the rule itself
- [x] `backend/crates/apps/coffret-device/src/library_dir.rs` still resolves the default to `coffret`, not `coffret-dev` (the binary's default is the production directory and is not moved)
- [x] `scripts/dev.sh` passes `make shell-lint` with the `prod` behaviour in it

### Before merge (verified outside the check command)
- [x] In a scratch clone on a branch (or with a dirty tree), `make prod` refuses before building and says why; on `main` with a clean tree it fast-forwards, builds, and starts the server and `vite preview`, and `make down` stops both — an agent runs this against a Library in a scratch `COFFRET_STATE_DIR` on the S3 store that `e2e-it.sh` uses, or reports the exact refusal and start-up lines if no Library can be made without a consent
- [x] `make dev` in this checkout, with `~/.local/state/coffret-dev` absent, names that directory (not `~/.local/state/coffret`) when it refuses an unknown Library
- [ ] Needs a person: in the production checkout, with its `local.mk` naming the default state directory and `PORT`, `make prod` serves the person's Library at `http://localhost:4173/` while `make dev` in a development checkout serves the development Library at `http://localhost:5173/` at the same time
