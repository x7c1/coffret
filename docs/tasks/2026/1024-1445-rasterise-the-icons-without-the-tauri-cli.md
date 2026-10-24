---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && ! grep -qi "tauri" scripts/gen-icons.sh Makefile docs/guides/environments.md'
assignee: null
branch: task/1024-1445-rasterise-the-icons-without-the-tauri-cli
created_at: 2026-10-24T05:45:00Z
updated_at: 2026-10-24T11:13:04Z
---

# build(icons): rasterise the icons without the Tauri CLI

## Overview

The Tauri app is gone, but `scripts/gen-icons.sh` (the `make icons` entry
point) still rasterises every icon with `cargo tauri icon`, so regenerating
the icons needs `tauri-cli` installed (`Makefile`, the `icons` target's
comment; `docs/guides/environments.md`, the paragraph on the script). Replace
it, so nothing in the repository depends on Tauri any more.

### Decision (already made; implement it)

- Rasterise with `resvg` — the renderer `cargo tauri icon` itself uses, so
  the pixels should not move. Prefer the `resvg` CLI installed once with
  `cargo install resvg --version <pinned> --locked`, checked for at the top of
  the script as `cargo tauri` is today; a small binary in the workspace built
  on the `resvg` crate is acceptable if the CLI cannot write a size the
  script needs.
- Write the `.icns` from the PNGs in python3, which the script already
  needs (it sorts the `.icns` entries today). Do not use `iconutil`: the
  script must keep working on Linux.
- Keep every output path, size and the "running it again writes the same
  bytes" property in the script's header. Update the header, the `icons`
  target's comment and the guide's paragraph to name the new tool.

### Out of scope

- Changing any icon's design, the sources under `assets/icon/`, or the
  Icon Composer icon (`packaging/icon.icon`).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/gen-icons.sh`, the `Makefile` and `docs/guides/environments.md`
      no longer mention Tauri (gate appended), and the script passes the
      shell lint in `make check`.

### Before merge (verified outside the check command)

- [ ] On this machine, with the new tool installed, `make icons` runs, and a
      second run leaves `git status` clean (same bytes). Every regenerated
      PNG is compared with the one on `main` by decoded pixels; the merging
      session records the largest per-channel difference per file in the PR
      and commits the regenerated files only where they differ.
- [ ] The regenerated `icon.icns` lists the same entry types and sizes as the
      one on `main` (recorded in the PR).
