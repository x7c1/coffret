---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment, error-type-design, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "pub prefix: Option<EntryPath>" backend/crates/domain/coffret-usecase/src/refused_root.rs && grep -q "fn prefix(&self)" backend/crates/domain/coffret-usecase/src/fetch/local_place.rs && grep -q "prefix: Option<EntryPath>" backend/crates/apps/coffret-device/src/error.rs && grep -q "prefix: Option<EntryPath>" backend/crates/apps/coffret-device/src/finding.rs && ! grep -q "pub(crate) const REFUSED_ROOT" backend/crates/apps/coffret-server/src/api_error/mod.rs && grep -q "fn refused_root_said" backend/crates/apps/coffret-server/src/api_error/mod.rs && grep -q "fn a_refused_root_names_the_mapping_in_the_sentence" backend/crates/apps/coffret-server/tests/routes.rs'
assignee: null
branch: task/0912-0454-name-the-mapping-a-refused-root-refusal-is-about
created_at: 2026-09-12T04:54:18Z
updated_at: 2026-09-12T07:21:54Z
---

# feat(backend): name the mapping in the refusal a refused root is answered with

## Overview

**This is a behaviour change a person notices.** The sentence a browser is
shown when a mapped root will not vouch for itself changes, and the value
that sentence is written from grows a field. Nothing about *when* a root is
refused changes.

EP-13 requires that "a refusal names the mapping and the reason"
(`docs/spec/entry-path/README.md:193`, the bullet beginning "The device
**refuses to place** when"). The reason is named. The mapping is not. What
reaches the browser today is one fixed sentence,
`backend/crates/apps/coffret-server/src/api_error/mod.rs:487`:

```
a folder this device maps is not the folder it was set up against, so nothing
was put into it; record the mapping again with `coffret map`
```

*"a folder this device maps"* is the whole of it. A device with more than one
mapping leaves the person holding a sentence that names no mapping and a
gesture — `coffret map` — that has to be pointed at one. The same sentence is
what a fill reports as a finding (`noted.rs:108`), so the gap is the same in
both places a person meets this state.

The fix is to carry the mapping's **Library-side prefix** in the refusal and
name it in the sentence. The prefix is the right half of the pair: a top-level
Entry Path component chosen by whoever recorded the mapping, a name inside the
Library rather than a path on this machine.

### Why the prefix may be said, and where it still may not

EL-1 (`docs/spec/event-logging/README.md:19`) forbids "an Entry Path, a local
path or filename, …" **in a diagnostic event**, and its second sentence is the
permission this change stands on: *"A person-facing refusal may identify a
file or Library that person owns; that rendering is not reused for an event."*
So an Entry Path component is forbidden in an event and permitted in the
sentence a person reads. The repository already reads it that way:
`FetchError::ReservedComponent`'s `component` is documented as reaching "them
in the message and never a diagnostic event (spec: EL-1)"
(`backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs:151-153`), and
`FetchError::UnmaterializablePath`'s message carries the whole Entry Path
(`fetch_error.rs:336-361`).

What stays out is unchanged and must stay out: the **local root**, and every
`Redacted` rendering. `RootRefused::redacted()`
(`coffret-usecase/src/refused_root.rs:145-157`) carries no prefix and must not
grow one; `ApiError`'s `cause` field — the log half — is built from that
rendering (`api_error/mod.rs:258`) and must stay free of the prefix.
`UnavailableRoot` is the precedent for the shape and for the wording of the
obligation: it already carries `prefix: Option<EntryPath>` beside its
`local_root`, documented as never travelling into an event
(`coffret-usecase/src/unavailable_root.rs:20-32`). `RefusedRoot` is its
companion by its own doc (`refused_root.rs:11-17`) and should look like it.

### Where the prefix comes from

`LocalPlace::new` is handed the whole `Mapping`
(`coffret-usecase/src/fetch/local_place.rs:49-55`) and keeps its `local_root`
and `expected_root_id` while dropping `Mapping::prefix`
(`coffret-usecase/src/device_state/mapping.rs:47-63`). So the prefix is
reachable at every site that decides a refusal, with **no new capability call
and no new dependency** — it only has to be kept.

The one place it is not reachable is the conversion the capability's own error
goes through. `DescentError::Refused { root, reason }`
(`coffret-usecase/src/descent_error.rs:68-73`) is the gateway's vocabulary and
correctly knows nothing about Entry Paths — `Destinations::reach` is handed the
root and the components apart for exactly that reason
(`coffret-usecase/src/destinations.rs:35-43`). So the prefix must be attached
on the domain side of that call, which means the two conversions
`FetchError::from_descent` (`fetch/fetch_error.rs:294`) and `Error::descent`
(`coffret-device/src/error.rs:973`) each need a prefix passed in. Every caller
of the first holds a `Target` and so a `LocalPlace`; `receive_file.rs:75-79`
holds the `LocalPlace` directly. The only caller with nothing to pass is
`IncomingFile` (`coffret-device/src/add/incoming_file.rs:72-140`, four sites),
which holds a `Destination` and a path and no mapping — so it must keep the
prefix as a field, handed to it at `open`.

Those four sites are also where the refused-root arm is unreachable in
practice: `DescentError::Refused` is produced only inside `reach`
(`gateway/coffret-local-fs/src/unix_destinations/vouch.rs:35` and the fake at
`coffret-usecase/src/in_memory_fs/state/roots.rs:43`), never by `create`,
`write`, `flush`, `publish`, or `rename` on an already-open `Destination`. The
arm exists because those operations share one error type. Narrowing that type
so a post-`reach` operation cannot report a refused root at all is the change
that would delete the arm; it is a capability-level restructure and is out of
scope here (see Out of scope).

## What to change

1. **`backend/crates/domain/coffret-usecase/src/refused_root.rs:32-38`** — add
   `pub prefix: Option<EntryPath>` as the first field of `RefusedRoot`, with a
   doc comment in `UnavailableRoot`'s voice:
   `/// The top-level component the mapping stands for, or None for the`
   `/// Library root.`
   Extend the struct's own doc at lines 24-26, which currently says only that
   the local root "never travels into a diagnostic event (spec: EL-1)", to say
   the same of the prefix and to say what the prefix is *for*: it is the half a
   refusal may name, because it is a name inside the Library rather than a path
   on this device. `unavailable_root.rs:20-22` is the sentence to mirror.
   `RootRefused` and its `Redacted` impl (lines 58-157) do not change.

2. **`backend/crates/domain/coffret-usecase/src/fetch/local_place.rs:26-55`** —
   add a `prefix: Option<EntryPath>` field, filled from `mapping.prefix.clone()`
   in `new`, documented the way `expected` is (lines 29-37): the mapping that
   says *where* is the mapping that says *which*, and a second reading of the
   mappings could answer the two from different rows. Expose
   `pub fn prefix(&self) -> Option<&EntryPath>` — `pub` because
   `coffret-device` reads it across the crate boundary. `descend` and `look`
   (lines 103-151) keep their signatures: the prefix does not cross into the
   capability.

3. **`backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs:171-176`** —
   add `prefix: Option<EntryPath>` to `FetchError::RefusedRoot`, ahead of
   `local_root`. Update the variant doc at lines 168-170 the way item 1 updates
   the struct's. Change `from_descent` (line 294) to
   `pub(super) fn from_descent(refused: DescentError, prefix: Option<&EntryPath>, path: &EntryPath) -> Self`
   and fill the new field in the `DescentError::Refused` arm (lines 300-303)
   from `prefix.cloned()`. In the `Display` arm at lines 375-381, keep the local
   root as the thing a terminal reader is sent to and name the mapping beside
   it — this rendering is for whoever is keeping the Library, and it already
   names the folder.

4. **`backend/crates/domain/coffret-usecase/src/fetch/placement.rs`** — at
   lines 141-146 fill the new field from `target.place.prefix().cloned()`; pass
   `target.place.prefix()` (or `self.target.place.prefix()`) at each
   `from_descent` call: lines 147, 153, 188, 217, 234, 271, 307. A small private
   helper on `Placement` that wraps `from_descent` with the target's own prefix
   and path is preferable to repeating the pair seven times.

5. **`backend/crates/domain/coffret-usecase/src/fetch/range_read.rs:154-159`** —
   the single-writer construction. `root` here is the `RefusedRoot` the
   placement handed back, so it now carries the prefix: pass `root.prefix`
   through rather than reading it a second time.

6. **`backend/crates/domain/coffret-usecase/src/fetch/select.rs:101`** — pass
   `target.place.prefix()` to `from_descent`. This call is on `look`, which
   never reports a refused root; it changes only because the signature does.

7. **`backend/crates/apps/coffret-device/src/error.rs:260-265`** — add
   `prefix: Option<EntryPath>` to `Error::RootRefused`, ahead of `root`. Change
   `descent` (line 973) to take `prefix: Option<&EntryPath>` beside `path` and
   fill the field in the `DescentError::Refused` arm (line 980). Name the
   mapping in the `Display` arm at lines 669-675, beside the folder it already
   names. The `Redacted` arm at lines 906-908 does **not** change — its comment
   at lines 903-905 states the rule this change must not break, and the test at
   line 1258 goes on asserting it.

8. **`backend/crates/apps/coffret-device/src/add/receive_file.rs:75-79`** — pass
   `place.prefix()` to `Error::descent`, and pass the prefix on to
   `IncomingFile::open` at line 80. Extend the `# Errors` paragraph at lines
   58-63 to say the refusal now names the mapping.

9. **`backend/crates/apps/coffret-device/src/add/incoming_file.rs:48-140`** — add
   a `prefix: Option<EntryPath>` field, taken as a parameter by `open`
   (line 72) and passed to the four `Error::descent` calls at lines 76, 96,
   127, and 139. Say in the field's doc why it is held: the refusal a descent
   reports names the mapping, and this value is the only thing here that knows
   which mapping the open folder came from.

10. **`backend/crates/apps/coffret-device/src/finding.rs:62-67`** — add
    `prefix: Option<EntryPath>` to `Finding::RefusedRoot`, ahead of
    `local_root`, and name it in the `Display` arm at lines 115-120. Leave
    `Finding::UnavailableRoot` (lines 43-48) alone: it has the same gap, and
    closing it is a separate change (see Out of scope).

11. **`backend/crates/apps/coffret-device/src/findings.rs:167-172`** — copy the
    prefix through in `refused`, as `local_root` and `reason` already are. The
    test fixtures below (around line 334) construct `RefusedRoot` and need the
    new field; give them a prefix rather than `None`, so the assertions are
    about a named mapping.

12. **`backend/crates/apps/coffret-server/src/api_error/mod.rs:251-260` and
    `475-489`** — replace the `REFUSED_ROOT` constant with a function, because
    the sentence is now a function of the mapping:

    ```rust
    pub(crate) fn refused_root_said(prefix: Option<&EntryPath>) -> String
    ```

    It answers, for a prefix of `albums`:

    ```
    the folder this device maps for "albums" is not the folder it was set up
    against, so nothing was put into it; record that mapping again with
    `coffret map`
    ```

    and for the Library-root mapping, where there is no component to name:

    ```
    the folder this device maps the Library root into is not the folder it was
    set up against, so nothing was put into it; record that mapping again with
    `coffret map`
    ```

    Spell the prefix with `{:?}` over `prefix.as_str()`, which is how every
    other person-facing message on these routes spells an Entry Path
    (`fetch_error.rs:336-371`). Say *that* mapping rather than *the* mapping:
    the sentence now names which one.

    `ApiError::refused_root` (line 251) takes the prefix beside the `Redacted`
    cause it already takes. Keep the two apart in the doc at lines 241-250: the
    prefix goes into the message and the cause into the log, and neither
    crosses.

    The function's own doc replaces lines 475-489. What is there now is also
    wrong as well as stale — *"which mapping it was reaches the person through
    the folder the line already names"* describes a line that names no folder.
    Say instead: the local path stays out (spec: EL-1) and the Library-side
    prefix goes in, because a name inside the Library is the person's own and a
    path on this device is not.

13. **`backend/crates/apps/coffret-server/src/api_error/from_error.rs:19` and
    `225`** — bind the prefix in each arm (`Error::RootRefused { ref prefix, .. }`,
    `FetchError::RefusedRoot { ref prefix, .. }`) and pass
    `prefix.as_ref()` beside the cause. Both arms already borrow the error for
    its redacted rendering, so nothing moves.

14. **`backend/crates/apps/coffret-server/src/noted.rs:53-56` and `93-118`** —
    `refused` takes the prefix and answers `String` rather than
    `&'static str`; `Noted::of`'s `Finding::RefusedRoot` arm passes it. Keep
    the sentence shared with the request's refusal — the reason given at lines
    102-107 is unchanged by this and is the reason the constant becomes one
    function rather than two strings. `path` stays `None`: the finding is about
    a mapping and not about one Entry.

15. **`backend/crates/apps/coffret-server/src/api_error/tests.rs:136-178` and
    `388-407`** — the constructions in both tests gain the new field. In
    `every_refused_root_reaches_the_browser_under_one_declined_reason`, keep the
    assertion that the folder is absent from the message and add one that the
    prefix is present. In `a_refused_root_records_which_case_it_was_and_no_path`,
    assert that the recorded line is unchanged — the prefix must not have
    reached the log.

16. **`backend/crates/apps/coffret-server/tests/routes.rs:518-569`** — the
    existing test asserts the message names the gesture. Add a test named
    `a_refused_root_names_the_mapping_in_the_sentence` that sets up a device
    with the marker rewritten as that test does (line 534) and asserts the
    `409 declined` body's `message` names the mapping's Library-side prefix and
    still names no local path. If the fixture maps only the Library root, the
    test should also cover the prefixed case, since the sentence differs.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] `RefusedRoot` carries the prefix:
  `grep -q "pub prefix: Option<EntryPath>" backend/crates/domain/coffret-usecase/src/refused_root.rs`
- [x] `LocalPlace` answers for it:
  `grep -q "fn prefix(&self)" backend/crates/domain/coffret-usecase/src/fetch/local_place.rs`
- [x] the device layer's refusal carries it:
  `grep -q "prefix: Option<EntryPath>" backend/crates/apps/coffret-device/src/error.rs`
- [x] and so does the finding a run reports:
  `grep -q "prefix: Option<EntryPath>" backend/crates/apps/coffret-device/src/finding.rs`
- [x] the sentence is no longer one fixed string:
  `! grep -q "pub(crate) const REFUSED_ROOT" backend/crates/apps/coffret-server/src/api_error/mod.rs`
- [x] it is written from the mapping instead:
  `grep -q "fn refused_root_said" backend/crates/apps/coffret-server/src/api_error/mod.rs`
- [x] and a route test holds the wire to it:
  `grep -q "fn a_refused_root_names_the_mapping_in_the_sentence" backend/crates/apps/coffret-server/tests/routes.rs`

## Out of scope

- **When a drop into a refused root is refused.** The upload route refuses
  such a part per part and reads the rest of the request; that is a separate
  change to the route's control flow and does not touch the payload this one
  changes.
- **`Finding::UnavailableRoot`'s own missing prefix.** `UnavailableRoot`
  already carries a prefix in the use case
  (`coffret-usecase/src/unavailable_root.rs:27`) and the finding drops it
  (`coffret-device/src/finding.rs:43-48`), so the sentence at
  `noted.rs:81-91` names no mapping either. The same gap, under EP-12 rather
  than EP-13, and worth closing on its own so that the two sentences are
  reviewed side by side.
- **Narrowing the capability's error type.** `DescentError::Refused` is
  produced only by `Destinations::reach`, yet every `Destination` operation
  reports the same type, which is why four unreachable arms have to be fed a
  prefix. Splitting the type so a post-`reach` operation cannot report a
  refused root would delete those arms; it changes the capability's surface
  and belongs in its own change.
- **Anything in `frontend/`.** The wire shape does not change: the browser
  reads `message` verbatim (`frontend/packages/gateway/api/src/refusal.ts:163-169`)
  and branches on `reason`, which stays `refused_root`.
- **The redacted renderings.** No diagnostic event gains a field. Any change
  to `RootRefused::redacted` or to `ApiError`'s `cause` is out of this change.
