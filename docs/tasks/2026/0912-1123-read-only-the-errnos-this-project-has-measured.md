---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [concept-alignment, completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "Errno::MLINK" backend/crates/apps backend/crates/gateway/coffret-local-fs/src/unix_destinations && ! grep -rq "EMLINK" backend/crates/apps backend/crates/gateway/coffret-local-fs/src/unix_destinations && grep -q "EMLINK" backend/crates/gateway/coffret-local-fs/src/lib.rs && grep -q "EFTYPE" backend/crates/gateway/coffret-local-fs/src/lib.rs && grep -q "compile_error!" backend/crates/gateway/coffret-local-fs/src/lib.rs && grep -q "target_vendor" backend/crates/gateway/coffret-local-fs/src/lib.rs && grep -q "fn a_symbolic_link_at_a_reserved_name_is_read_as_blocked" backend/crates/gateway/coffret-local-fs/src/unix_destinations/mod.rs'
assignee: null
branch: task/0912-1123-read-only-the-errnos-this-project-has-measured
created_at: 2026-09-12T11:23:26Z
updated_at: 2026-09-13T05:55:57Z
---

# refactor(backend): read only the errnos this project has measured

## Overview

Six match arms read `EMLINK` as a second spelling of `ELOOP`, for a platform
family this project does not target. They were written from a reading of the
BSDs' documentation rather than from a host, and no test drives them — the test
beside the registration says so in as many words:

> What this proves is one of the two spellings. On Linux `O_NOFOLLOW` reports
> `ELOOP` for a link at either name, so no case here drives the `EMLINK` arm
> beside it — exactly as none drives the placement side's, which reads both for
> the same reason. That arm carries a comment naming the platform that spells it
> that way instead, and a host that does is where the other half is confirmed.

Both platforms this project runs on were measured on 2026-09-12, and neither
reports `EMLINK`:

| what was opened, with which flags | Linux | macOS (Darwin 25.6.0) |
| --- | --- | --- |
| a symbolic link, `O_RDONLY\|O_NOFOLLOW` | `ELOOP` | `ELOOP` |
| a symbolic link, `O_RDONLY\|O_NOFOLLOW\|O_DIRECTORY` | `ELOOP` | **`ENOTDIR`** |

So the arm is unreachable code kept for an unsupported port, and this change
takes it out. **What must not be lost is the knowledge it carried** — that a
port to another Unix has to settle which errno its kernel reports at a reserved
name, because the reading is what separates "something else is standing at the
reserved name" from "the disk went wrong", and a port that gets an errno this
code does not read would report a local I/O failure where the rule names a
verdict (spec: EP-13, EP-14).

Two mechanisms replace the six arms, so that the knowledge is kept where it is
met rather than where it happens to have been written:

1. **A platform gate that stops the build.** The reading lives in
   `coffret-local-fs`, and every binary reaches it, so one `compile_error!`
   there is enough: a build for a target nobody measured fails with a message
   saying what has to be settled first. It also replaces what a Windows build
   currently produces — a pile of type errors out of `rustix` — with one
   sentence.
2. **A test that pins what this host reports.** The measurement above is what
   the arms should have been written from. As a test beside `refusal`, it says
   that whatever the platform reports for a link at a reserved name is read as a
   fence rather than as an I/O failure, and it fails on a platform whose kernel
   answers something else — which is the same message the gate carries, arriving
   from the other side.

The crate documentation gains the porting note the message points at. It already
claims the reading: "What it does decide, and nothing above it may, is what an
errno means … Reading `ELOOP` and `ENOTDIR` to know that is this crate's alone."

The macOS row above also fixes a claim three comments make. They tie each errno
to one cause — `O_NOFOLLOW` reports `ELOOP` for a link, `O_DIRECTORY` reports
`ENOTDIR` for anything that is not a folder — and the `.coffret` side passes both
flags, so on macOS a link arrives as `ENOTDIR`. The verdict is right, because all
of them are read as one; the explanation of which flag produces which errno is
not, and it is what a reader would reason from.

## What to change

### 1. The six arms

`backend/crates/gateway/coffret-local-fs/src/unix_destinations/mod.rs:146` —
`refusal`, the one place the gateway turns an errno into a verdict:

```rust
if cause == Errno::LOOP || cause == Errno::NOTDIR || cause == Errno::MLINK {
```

Drop the third comparison. Its doc comment above says `EMLINK` "is the same
verdict as `ELOOP` on the BSDs"; that sentence goes, and the sentence that ties
each of the other two to one flag is the one the macOS row corrects — say instead
that both are read as one verdict, and that which of them a kernel reports for a
link depends on the platform and on whether `O_DIRECTORY` was passed.

`backend/crates/gateway/coffret-local-fs/src/unix_destinations/vouch.rs:56` and
`:81` — the management area's descent and the marker's open:

```rust
Err(Errno::LOOP | Errno::MLINK | Errno::NOTDIR) => {
Err(Errno::LOOP | Errno::MLINK) => return refused(RootRefused::MarkerNotARegularFile),
```

Drop `Errno::MLINK` from both. The comment at `:50` names "`EMLINK` where the
BSDs spell it that way, the reading the descent's own `refusal` makes of the same
errno beside this"; the cross-reference to `refusal` is worth keeping, the
`EMLINK` clause is not. The comment at `:79` says "in either spelling", which is
now one spelling.

`backend/crates/apps/coffret-device/src/mapping/root_marker/management_area.rs:60`
and `:83` — the same two, on the registration side. The doc comment at `:46`
carries the flag-to-errno attribution and the sentence "Both spellings of the
link are read here and after the racing `mkdirat` below, because the placement
side reads both": the *reason* survives (registration and placement must make the
same verdict of the same errnos, or a registration reports an I/O failure where
the rule names a verdict), the count of spellings does not.

`backend/crates/apps/coffret-device/src/mapping/root_marker/read_marker.rs:43` —
the marker's open. Its comment says "The link the open turned away, in either
spelling: `O_NOFOLLOW` reports `ELOOP` — `EMLINK` where the BSDs spell it that
way", and ends "the placement side makes the same verdict of the same two
errnos". One errno now.

`backend/crates/apps/coffret-device/src/mapping/tests.rs:391` — the comment
quoted in the Overview. What it says about the test is still true: the case
drives the link at either name through one errno. What goes is the paragraph
about the undriven arm and the host that would confirm it, since there is no
such arm left to excuse. Say instead that the two sides read the same errno, and
that a platform reporting another is what the gateway's own contract test and
the platform gate are for.

### 2. The platform gate

In `backend/crates/gateway/coffret-local-fs/src/lib.rs`, beside the crate
documentation:

```rust
#[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
compile_error!(
    "coffret's local filesystem gateway reads the errno a reserved name reports, \
     and only Linux and macOS have been measured: ELOOP for a symbolic link, and \
     ENOTDIR once O_DIRECTORY is passed beside O_NOFOLLOW. Porting to another \
     Unix means measuring its own on a host of that kind and reading it here — \
     FreeBSD documents EMLINK for the link and NetBSD EFTYPE, and neither is \
     read. See this crate's documentation for the rest of what a port settles."
);
```

`target_vendor = "apple"` is the spelling `unix_mapped_roots/list_folder.rs`
already uses for the birth-time split, so the two agree on how this project
names the platform.

One gate is enough for every crate: `coffret-device` reads the same errnos with
its own `rustix` calls, but it depends on this crate, so no build of it reaches a
compiler without passing through here. **Do not add a second `compile_error!` to
`coffret-device`** — two gates would be two places to keep in agreement, and the
message belongs where the reading is claimed.

### 3. The porting note

A section in the same crate documentation, which the message above points at.
What a port to another Unix has to settle, each with what is already known:

- **The errno a reserved name reports.** The table in the Overview, and the two
  documented values this code does not read (`EMLINK` on FreeBSD, `EFTYPE` on
  NetBSD — read out of their manuals, not measured). A port adds its own to
  `refusal` and to the two `root_marker` opens together, because a registration
  and a placement that disagreed would refuse in different vocabularies.
- **Whether the volume folds case.** macOS's APFS does by default, and the
  reserved name is compared exactly (spec: EP-14), so the name and a case variant
  of it are one directory on disk that only one of them is recognized as. This is
  not settled — say so, rather than implying a port inherits a working rule.
- **Whether a directory listing hands back the spelling it was given.** The walk
  composes every name to NFC at the boundary (spec: EP-1) while a reader reopens
  the composed spelling, which holds on a volume whose lookup ignores the
  difference — APFS's does. A volume that preserves the spelling *and*
  distinguishes it in lookup makes those two disagree.
- **Whether `st_birthtime` exists.** Already answered per platform by
  `unix_mapped_roots/list_folder.rs`; named here so the list is the whole of it.

Write it as what a port must decide, not as a list of platform trivia: each line
is a question whose answer changes what this crate does.

### 4. The contract test

A `#[cfg(test)]` module in `unix_destinations/mod.rs`, beside `refusal` — the
crate keeps unit tests in `src` this way already (`local_times.rs`). It is a unit
test rather than one under `tests/` because `refusal` is private and it is
`refusal`, not a public entry point, that holds the reading.

```rust
#[test]
fn a_symbolic_link_at_a_reserved_name_is_read_as_blocked() {
```

Make a temporary directory, a symbolic link in it pointing at a directory, and
one pointing at a regular file. Open each with the flags the two sides use —
`O_RDONLY | O_NOFOLLOW | O_DIRECTORY` for the management area,
`O_RDONLY | O_NOFOLLOW | O_NONBLOCK` for the marker — and assert that the errno
each returns is read by `refusal` as `DescentError::Blocked`. Assert on the
verdict, not on the errno: the test is about what this crate concludes, and a
platform whose kernel reports another value is exactly the case where the
conclusion must be looked at again. Name the errno in the failure message so the
port knows what to add.

`tempfile` is already a dev-dependency, and `rustix` is a dependency, so a unit
test reaches both.

## Acceptance criteria

### Automated (pipeline-verified)

- [ ] `make check` passes for the workspaces this change touches. Doc comments
  are compiled and linted, so a malformed rustdoc line in the porting note fails
  here, and the new test runs as part of it.
- [ ] No spelling of `EMLINK` remains where the arms and their comments were:
  `! grep -rq "Errno::MLINK" backend/crates/apps backend/crates/gateway/coffret-local-fs/src/unix_destinations` and
  `! grep -rq "EMLINK" backend/crates/apps backend/crates/gateway/coffret-local-fs/src/unix_destinations`.
  Two patterns rather than one, and neither of them the bare `MLINK`: the
  string `SYMLINK` contains `MLINK`, so a gate on the bare spelling also
  matches `AT_SYMLINK_NOFOLLOW` and `AtFlags::SYMLINK_NOFOLLOW` in
  `backend/crates/gateway/coffret-local-fs/src/unix_destinations/look.rs`, which this change must not touch — it could then only be
  satisfied by deleting a correct flag. `Errno::MLINK` catches the arms and
  `EMLINK` catches their prose; neither matches a `SYMLINK` line.
  The pathspec deliberately spares
  `backend/crates/gateway/coffret-local-fs/src/lib.rs`, which is where the name
  must survive.
- [ ] The knowledge survives in exactly the one place the build points at:
  `grep -q "EMLINK" backend/crates/gateway/coffret-local-fs/src/lib.rs`
  and `grep -q "EFTYPE" backend/crates/gateway/coffret-local-fs/src/lib.rs`.
  `EFTYPE` appears nowhere in the tree today, so this one fails unless the note
  names NetBSD's value as well — the trap is a port reading a *documented* errno
  that is not in the match.
- [ ] The gate exists and is a platform gate rather than prose:
  `grep -q "compile_error!" backend/crates/gateway/coffret-local-fs/src/lib.rs`
  and `grep -q "target_vendor" backend/crates/gateway/coffret-local-fs/src/lib.rs`.
- [ ] The contract test exists under the name the pipeline can find:
  `grep -q "fn a_symbolic_link_at_a_reserved_name_is_read_as_blocked" backend/crates/gateway/coffret-local-fs/src/unix_destinations/mod.rs`.
- [ ] The registration test that named the undriven arm still passes:
  `a_root_whose_marker_is_a_symbolic_link_is_refused`. Its comment changes; its
  behaviour must not.

### Manual

None. Both platforms' errnos were measured before this task was written, and the
values are in the Overview.

## Out of scope

- **The case-folding hole itself.** That the reserved name is defeated by a
  volume that folds case is a defect of its own, with a register change behind
  it (spec: EP-14 decides by name). This task only stops the porting note from
  implying the rule is settled; it does not change `is_management_area`.
- **Verifying FreeBSD or NetBSD.** Their values are quoted from their manuals and
  marked as such. Measuring them is what the gate asks of whoever ports.
- **Widening or narrowing the verdict.** `ELOOP` and `ENOTDIR` keep meaning
  exactly what they mean today; `DescentError::Blocked`, `MarkerNotARegularFile`
  and `ManagementAreaNotADirectory` are untouched.
- **The `#[cfg(unix)]` question.** Whether the workspace should declare Unix-only
  support in its manifests, rather than at this one crate's compile gate, is a
  separate decision.
