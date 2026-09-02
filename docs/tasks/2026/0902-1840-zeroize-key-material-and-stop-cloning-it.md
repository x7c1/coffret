---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'zeroize' backend/crates/domain/coffret-model/Cargo.toml && ! grep -q 'derive(Clone)' backend/crates/domain/coffret-model/src/master_key.rs && ! grep -q 'derive(Clone)' backend/crates/domain/coffret-model/src/container_key.rs"
assignee: null
branch: task/0902-1840-zeroize-key-material-and-stop-cloning-it
created_at: 2026-09-02T09:40:00Z
updated_at: 2026-09-02T12:20:00Z
---

# fix(model): zeroize key material and stop cloning it

## Overview

DK-7 promises that key material lives in a type that overwrites itself
when dropped and is never copied outside that type. The code says
otherwise, in its own words: `coffret-model`'s `MasterKey` and
`ContainerKey` carry doc notes admitting zeroization is not implemented,
and both derive `Clone` — as do the other secret-bearing values along the
unlock path (`coffret-format`'s `PurposeKey` and the unlocked form of the
stored Master Key, `coffret-usecase`'s `LibraryKeys`, and the Passphrase
the shell reads). Every derive is an invitation for key bytes to multiply
across the heap, and every drop leaves them lying in freed memory. The
spec register is the contract; make the code keep it.

1. **Inventory first, then close the set.** Walk the unlock path from the
   Passphrase to the per-Container keys and list every type that holds
   secret bytes — including the intermediate plaintext buffers between
   derivations (KDF output, decrypted key envelopes, the bytes a
   `SecretBox`-style wrapper would guard). The inventory is the deliverable
   that makes the rest checkable: put it in the module doc of the type
   that anchors the hierarchy, so the next key-bearing type has a list to
   join.
2. **Zeroize on drop.** Give every type in the inventory drop-zeroization
   via the `zeroize` crate (`ZeroizeOnDrop` or a manual `Drop` where the
   layout demands it). Where a wrapper takes raw bytes in or hands raw
   bytes out, shrink that window: take ownership of buffers and zeroize
   them at the boundary rather than copying out of them. Be honest about
   the limits — Rust cannot stop the OS from paging or a debugger from
   reading; the promise is that *this process does not keep readable
   copies it no longer needs*, which is exactly DK-7's wording. Do not
   weaken the spec text instead of implementing it.
3. **Remove `Clone` from secret-bearing types.** A caller that compiles
   only because keys are cloneable is a copy nobody audits. Replace each
   clone site with borrowing, moving, or explicitly shared ownership
   (`Arc<...>` around the zeroizing type, so the copy count stays one) —
   whichever the call site actually needs. If some type genuinely cannot
   lose `Clone` yet, keep it with a comment saying which caller forces it
   and why that copy is sound; do not keep a derive silently.
4. **Prove it where proof is possible.** Compile-time: the types in the
   inventory implement `ZeroizeOnDrop` (or document their manual `Drop`)
   and no longer implement `Clone` — a small trait-assertion test pins
   both so a future derive fails loudly. The encode/decode and interop
   paths must behave byte-identically: `make check` runs the interop
   suite, which is the cross-implementation proof that wrapping and
   zeroizing changed nothing observable.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Every type in the secret-bearing inventory zeroizes its bytes on
      drop, and the inventory is written where the anchor type's module
      doc lives (the `zeroize` dependency gate in `check_command` pins
      the crate; the two `! grep 'derive(Clone)'` gates pin the model
      types).
- [x] `Clone` is gone from the inventory's types, or each remaining
      implementation carries a comment naming the caller that forces it;
      a trait-assertion test fails compilation if `Clone` or the
      zeroize-on-drop bound regresses.
- [x] Intermediate plaintext key buffers (KDF outputs, decrypted
      envelope payloads) are owned and zeroized at their boundaries, not
      copied out of and forgotten.
- [x] `make check` passes — the interop suite proves the storage format
      and cross-implementation behavior are byte-identical.

## Out of scope

- The lock lifecycle of the long-running server (explicit lock, idle
  lock, and what they do to in-flight work) — that is its own change on
  top of this groundwork.
- The OAuth token cache's custody and the browser/UI side.
- Memory the TLS and crypto libraries manage internally.
- Log redaction — nothing here changes what is logged.
