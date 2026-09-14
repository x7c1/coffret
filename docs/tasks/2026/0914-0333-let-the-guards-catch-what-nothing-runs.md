---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test -f backend/deny.toml && grep -q "cargo doc" .github/workflows/ci.yml && grep -q "cargo-deny" .github/workflows/ci.yml'
assignee: null
branch: task/0914-0333-let-the-guards-catch-what-nothing-runs
created_at: 2026-09-14T03:33:00Z
updated_at: 2026-09-14T04:22:45Z
---

# ci(backend): let the guards catch what nothing runs today

## Overview

Two things this repository depends on are checked by nobody.

### 1. `cargo doc` runs nowhere

`make check` (`Makefile`) runs `cargo fmt --all -- --check && cargo build &&
cargo test && cargo clippy --all-targets -- -D warnings`, and the `backend` job
in `.github/workflows/ci.yml` runs the same four steps. **Neither runs
`cargo doc`.** So a broken intra-doc link — the one failure mode a
documentation-only change actually has, in a repository whose doc comments carry
the reasoning behind nearly every decision — reaches `main` with everything
green.

It is not theoretical: there are three warnings on `main` right now, all the
same shape, public documentation linking to a private item:

```
crates/gateway/coffret-sqlite-index/src/sqlite_index.rs:42  `SqliteIndex` → `BUSY_TIMEOUT`
crates/apps/coffret-server/src/state.rs:16                  `ServerState` → `Custody`
crates/apps/coffret-server/src/state.rs:17                  `ServerState` → `Self::unlocked`
```

Fix those three, then add the step that keeps the next three from arriving.
Warnings have to fail the run — a step that prints them and exits zero is the
same silence with more output.

For each of the three, the question is whether the item should be public or the
link should not be one. **Prefer un-linking**: something is private because
somebody decided it was, and this change is not the place to widen an API.

### 2. Nothing audits the dependencies

There is no `deny.toml` and `cargo-deny` runs nowhere, so an advisory against a
crate this product ships, a duplicate version pulled in twice, or a dependency
arriving from somewhere other than crates.io all land unremarked. For a product
whose whole claim is that it holds other people's files under encryption, that
is the guard with the most to say.

Add `cargo-deny` to CI with a `deny.toml` covering advisories, bans, licenses
and sources.

**The licence half has already been surveyed and is not the hard part.** The
dependency tree carries `AGPL-3.0-only` on fourteen packages — all of them
coffret's own crates, because `backend/Cargo.toml` sets
`license = "AGPL-3.0-only"` for the workspace. Every third-party licence is
permissive, among them `MIT OR Apache-2.0`, `MIT`, `Apache-2.0`,
`Unicode-3.0`, `CDLA-Permissive-2.0`, `Apache-2.0 WITH LLVM-exception`, `Zlib`,
`ISC`, `Unlicense`, `CC0-1.0`, `MIT-0`, `BSD-2-Clause`, `BSD-3-Clause`, and
`OR` expressions that offer one of those. Treat that as a starting point rather
than a closed set and count them again — it was assembled from the most common
first and its tail is where a surprise would be. Thirteen packages spell theirs
with a slash rather than `OR`, which is not SPDX and which some versions of
`cargo-deny` will not parse — if that bites, say so in the configuration rather
than working around it silently.

If a licence needs an exception, record it in `deny.toml` with the reason it is
acceptable and carry on. **Stop and report instead** only if a dependency
carries a licence that would actually constrain how this product may be
distributed — that is a decision about the product, not about a lint.

## What this change has to decide

- **Where `cargo doc` goes.** It belongs in both `make check` and CI, because
  the point is that a developer finds it before the PR does. Whether the CI step
  is its own or joins the `backend` job is yours.
- **How `cargo-deny` runs in CI.** An action pinned to a version, or
  `cargo install cargo-deny` in the job. The first is faster and pins more
  clearly; the second adds no third-party action to the supply chain of a
  security product. Pick one and say in the workflow why.
- **Whether `cargo-deny` also belongs in `make check`.** It fetches an advisory
  database, so it is not free on every local run. Decide, and say which way in
  the `Makefile` comment if it stays out.

## Out of scope

- **Fixing anything `cargo-deny` reports beyond configuration.** If it finds an
  advisory that needs a dependency bumped, record what it found and leave the
  bump to its own change — that is a dependency update with its own testing, not
  part of adding the guard.
- **Three other guard-adjacent leftovers**: `UnreachablePlace`'s TypeScript
  round-trip, the clap stderr path and the unexported `Zeroizing` left over from
  an earlier change, and the vocabulary cleanup of committed task files. Each is
  its own change; none of them is about a guard that does not run.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `cargo doc` runs in `make check` and in CI, and a rustdoc warning fails
      both.
- [x] The three existing warnings are gone, and each was fixed by deciding
      whether the item is public rather than by silencing the lint.
- [x] `deny.toml` exists and covers advisories, bans, licenses and sources, and
      `cargo-deny` runs in CI.
- [x] Every allowance in `deny.toml` says why it is there. A bare list of
      licence identifiers with no reason is the configuration equivalent of the
      silence this change is removing.
- [x] `make check` still passes, and nothing in it was weakened to make room.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here is observable at runtime: every change is to a build guard or
      a doc link. No manual check is needed.
