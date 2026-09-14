---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make deny && grep -A 1 -x ''name = "chacha20"'' backend/Cargo.lock | grep -q ''version = "0.10.2"'''
assignee: null
branch: task/0914-0428-move-off-the-yanked-cipher
created_at: 2026-09-14T04:28:00Z
updated_at: 2026-09-14T04:57:54Z
---

# build(backend): move off the yanked cipher release

## Overview

`backend/Cargo.lock` resolves `chacha20` to `0.10.1`, which its authors yanked.
So did `0.10.0`. `0.10.2` has been the live release since 2026-08-27.

This is the cipher this product encrypts with. `chacha20` reaches the tree
through exactly one edge — `chacha20poly1305 0.11.0` → `coffret-format` — and
`coffret-format` is where every Entry in a Library is sealed under
XChaCha20-Poly1305 (`docs/spec/format/README.md`, `FM-1`).

The dependency audit added in the previous change records the yank and waives
it, saying in `backend/deny.toml` that the move belongs in its own pull request
where the test suite and the Rust/TypeScript interop exchange are run against
it. This is that pull request.

### What the move should be

`chacha20poly1305 0.11.0` requires `chacha20 ^0.10`, so `0.10.2` is reachable
without touching a manifest: the change should be `Cargo.lock` and nothing else.
**If it turns out to need more than that, stop and report** — a manifest edit
here would mean the two releases are not the drop-in the version requirement
says they are, and that is a different change with a different risk.

Then **remove the ignore from `backend/deny.toml`**. It is written to be
removed: it names the version, so `cargo-deny` reports it as unused the moment
the lockfile moves, and its own comment says to take it out then. Leaving it
would turn a dated decision into a standing one, which is the thing that comment
exists to prevent.

### What proves it

The interop exchange is the guard that matters, and it is a good one by
accident of design: the TypeScript side does not use this crate at all. It
implements XChaCha20-Poly1305 through `@noble/ciphers`
(`frontend/packages/domain/format/src/internal/aead.ts`), so the two sides are
independent implementations of the same construction. If the Rust release
changed anything a reader of the format can observe, the exchange stops
round-tripping. `make check` runs it.

### What is missing from the record

`backend/deny.toml` says the version was yanked and what to do about it, but not
**why** it was yanked — and the file's own text argues that failing on a yank is
"what makes somebody read why it was pulled". Nobody has read it yet; the
crates.io API does not carry yank reasons.

Find out, from the crate's own repository — its changelog, its release notes, or
the commit or issue that accompanied the yank. Record what you find in the pull
request body, and say plainly if you cannot find it rather than guessing. A yank
for a build break and a yank for a cryptographic defect are not the same news,
and this product's readers deserve to know which this was.

## Out of scope

- **Any other dependency update.** The lockfile has other crates behind their
  latest releases; moving them is not this change and would obscure what the
  interop exchange is testing here.
- **`chacha20poly1305` itself.** Its `0.11.0` is not yanked, and the version
  requirement it already states is what makes this a lockfile-only move.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `backend/Cargo.lock` resolves `chacha20` to `0.10.2`, and no manifest
      changed to get there.
- [x] The `chacha20` entry is gone from `backend/deny.toml`'s `ignore` list, and
      `make deny` passes without it.
- [x] `make check` passes, the Rust/TypeScript interop exchange included.
- [x] Nothing else in `backend/Cargo.lock` moved.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here needs hardware. The cipher is exercised by the test suite and
      by the interop exchange, both of which run in `make check`.
