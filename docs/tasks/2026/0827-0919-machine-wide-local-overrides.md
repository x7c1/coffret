---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && git grep -qF -- '-include $(HOME)/.config/coffret/local.mk' -- Makefile && git grep -qF -- '-include local.mk' -- Makefile"
assignee: null
branch: task/0827-0919-machine-wide-local-overrides
created_at: 2026-08-27T09:19:09Z
updated_at: 2026-08-27T09:34:00Z
---

# build: also read machine-wide overrides from ~/.config/coffret/local.mk

## Overview

`local.mk` (added in `docs/tasks/2026/0827-0838-local-makefile-overrides.md`)
is per-checkout: it lives beside the `Makefile` and is gitignored. That shape
has a gap for git-worktree workflows — a worktree starts with tracked files
only, so an override that is really about the *machine* (a toolchain pin like
`export CC := /usr/bin/cc`) has to be recreated in every new worktree by hand.

Give machine-wide overrides a home that every checkout and worktree of this
repo reads automatically. In `Makefile`, extend the existing override block
(currently the comment plus `-include local.mk` after
`.DEFAULT_GOAL := help`) to:

```make
# Optional overrides, machine-wide then per-checkout (later wins): toolchain
# pins like `export CC := /usr/bin/cc`, parameter defaults, extra targets.
# Both absent on a fresh clone; `-include` skips a missing file silently.
-include $(HOME)/.config/coffret/local.mk
-include local.mk
```

The order is the point: the machine-wide file is read first, the per-checkout
`local.mk` second, so a checkout-local experiment can override the machine
default (for simple `:=` assignments, the later assignment wins). Use
`$(HOME)` rather than `~` — make does not tilde-expand include paths. The
fixed `$(HOME)/.config/coffret/` path is deliberate: resolving
`XDG_CONFIG_HOME` with fallback logic in make costs more lines than the
convention is worth here, and the comment names the exact path a reader
should create.

`.gitignore` needs no change (the machine-wide file lives outside the repo),
and behavior with neither file present stays identical.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `Makefile` contains both include lines in order — 
      `-include $(HOME)/.config/coffret/local.mk` first, `-include local.mk`
      second (the check command greps for both), placed where the single
      include was: after `.DEFAULT_GOAL := help` and before the first
      parameter assignment.
- [x] The comment block above them describes both files and the precedence
      (machine-wide then per-checkout, later wins).
- [x] `make check` still passes on a tree where neither override file is
      present in the repo — no target, variable default, or recipe changes.

## Out of scope

- **XDG_CONFIG_HOME resolution.** The path is fixed to
  `$(HOME)/.config/coffret/local.mk`; supporting the environment variable is
  extra make logic with no current user.
- **Committing any override file or example.** Both files stay untracked;
  the comment documents them.
