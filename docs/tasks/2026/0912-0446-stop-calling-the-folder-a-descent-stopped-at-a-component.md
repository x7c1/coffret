---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rq 'stopped_at: Option<PathBuf>' backend/crates/domain/coffret-usecase/src/ && grep -rq 'stopped_at: PathBuf' backend/crates/apps/coffret-device/src/ && ! grep -rq 'component: Option<PathBuf>' backend/crates/ && ! grep -rq 'component: PathBuf' backend/crates/ && ! grep -rq 'component: None' backend/crates/ && ! grep -rq 'component: Some(' backend/crates/ && ! grep -rq 'Blocked { path' backend/crates/ && ! grep -rq 'Blocked { ref path' backend/crates/ && ! grep -rq 'path: at.to_path_buf()' backend/crates/gateway/coffret-local-fs/src/ && ! grep -rq 'UnreachablePlace { component' backend/crates/ && ! grep -rq 'UnreachablePlace { path, component }' backend/crates/ && ! grep -rq 'fn component()' backend/crates/apps/coffret-server/src/ && grep -rq 'fn a_blocked_place_says_it_was_blocked_and_never_where_it_stopped' backend/crates/domain/coffret-usecase/src/ && ! grep -rq 'component it stopped at' backend/crates/domain/coffret-usecase/src/ && ! grep -rq 'component the walk stopped at' backend/crates/domain/coffret-usecase/src/ && ! grep -rq 'component the descent stopped at' backend/crates/domain/coffret-usecase/src/"
assignee: null
branch: task/0912-0446-stop-calling-the-folder-a-descent-stopped-at-a-component
created_at: 2026-09-12T04:46:11Z
updated_at: 2026-09-12T06:08:20Z
---

# refactor(backend): stop calling the folder a descent stopped at a component

## Overview

`component` means one thing in this codebase and is used for two. The Entry
Path concept fixes the first: an Entry Path is *"NFC and encoded as UTF-8, with
`/` between components"* (`docs/concepts/entry-path/README.md:7`) — a component
is a piece of a path the Library carries, which is the user's own name for part
of their file's place. The second use is a local `PathBuf`: the folder on *this
device* that a descent stopped at, which is a different kind of thing with a
different disclosure rule (a local path may never reach a diagnostic event,
spec: EL-1) and a different thing for a person to do about it.

`coffret-usecase/src/fetch/fetch_error.rs` holds both senses in one enum, a
few dozen lines apart:

- `UnmaterializablePath { path: EntryPath, component: Option<PathBuf> }`
  (`fetch_error.rs:109`, field at `:120`) — the local folder a descent stopped
  at, `None` where nothing on disk was reached. Its `Display` arm prints it as
  a folder (`fetch_error.rs:336`) and its `Redacted` arm prints only whether
  there was one, as `descent=blocked` or `descent=unspellable`
  (`fetch_error.rs:492`), because the folder itself may not be written down.
- `ReservedComponent { path: EntryPath, component: String }`
  (`fetch_error.rs:146`, field at `:154`) — a component of an Entry Path, the
  one name in it coffret keeps for itself, named to a person in the message
  (`fetch_error.rs:365`) and kept out of the log (`:503`).

Reading either variant's `component` requires remembering which one you are
looking at. The same crossing is repeated in three more places, and one of them
is the source the value comes from:

- `DescentError::Blocked { path: PathBuf }`
  (`coffret-usecase/src/descent_error.rs:46`, field at `:52`) — the field is
  called `path`, which is the *other* thing this enum's sibling carries
  (`Refused { root, reason }` at `:68`, and `Io(LocalIoError)` at `:76`, whose
  own value is a path too), while the doc on it calls it a component: *"The
  component the descent stopped at."* (`descent_error.rs:47`). The `Redacted`
  doc repeats it (`:121`) and so does the test comment at `:150`. This is the
  value every other site's is built from.
- `Surfaced::UnreachablePlace { path: EntryPath, component: PathBuf }`
  (`coffret-usecase/src/fetch/surfaced.rs:66`, field at `:75`) — a local folder
  called `component`, in an enum whose next-but-one variant is
  `ReservedComponent { path: EntryPath }` (`surfaced.rs:105`, the same
  distinction drawn by the variant name alone).
- `FindingReason::UnreachablePlace { component: PathBuf }`
  (`coffret-device/src/finding_reason.rs:64`, field at `:67`) — the same local
  folder one layer up, beside `FindingReason::ReservedComponent`
  (`finding_reason.rs:78`).

The field docs on three of the four already say the right word — *"The folder on
this device a descent stopped at"* (`fetch_error.rs:112`), *"The folder on this
device the descent stopped at"* (`surfaced.rs:69`, `finding_reason.rs:65`) — so
the name is the only thing out of step with the prose around it. This change
brings the name up to the prose, in one word, across every site.

**Nothing crosses the wire.** `coffret-usecase` has no `serde` dependency at all
(`backend/crates/domain/coffret-usecase/Cargo.toml`, `[dependencies]` lists
`coffret-format`, `coffret-model`, `async-trait`, `blake3`, `getrandom`,
`md-5`, `tokio`, `tracing`, `zeroize` and nothing else), so `FetchError`,
`DescentError` and `Surfaced` carry no derive and no attribute that could put a
field name in a JSON body. `coffret-device`'s `Finding` and `FindingReason`
carry none either (`findings.rs` and `finding_reason.rs` contain no `serde`
token; `FindingReason` derives `Debug, Clone, PartialEq, Eq` at
`finding_reason.rs:22`). What reaches the browser is the variant *name*
(`coffret-server/src/api_error/mod.rs:492`'s `name_of`, which answers
`"UnreachablePlace"`) and a sentence (`api_error/mod.rs:205`,
`noted.rs:151`) — neither of which this change touches. On the TypeScript side
`'UnreachablePlace'` appears only as that same variant name
(`frontend/packages/gateway/api/src/refusal.ts:90` and `:229`,
`refusal.test.ts:56`). **`frontend/` is therefore not in this change's file
set, and no `.ts` file needs editing.**

### The name

`stopped_at` for all four. Three reasons, in the order they decided it:

1. It is already this repository's own phrase for the thing. *"the folder the
   descent stopped at"* and *"the component the descent stopped at"* are
   written verbatim at `descent_error.rs:47`, `fetch_error.rs:112`,
   `surfaced.rs:69` and `finding_reason.rs:65`. The rename promotes existing
   vocabulary rather than introducing a word.
2. It reads as a place rather than as a piece of a path, which is the whole
   distinction being drawn: after the change `component` appears as a field
   name in exactly one place in the fetch vocabulary —
   `ReservedComponent { component: String }` (`fetch_error.rs:154`) — and it is
   the Entry Path sense the concept document defines.
3. It says the same thing the redacted label already says. That label names the
   *descent* (`descent=blocked` where a folder was reached and the descent
   stopped, `descent=unspellable` where no folder was reached at all,
   `fetch_error.rs:493`), and `stopped_at` names where that descent stopped, so
   the value and the log line speak one vocabulary. **Leave the label itself
   exactly as it is** — `descent=blocked` / `descent=unspellable` are asserted
   at `fetch_error.rs:622` and `:626`, at
   `coffret-server/src/api_error/tests.rs:355` and `:362`, and at
   `coffret-server/tests/routes.rs:2434`, and they are the worked example in
   `coffret-model/src/redacted.rs:28`. Renaming a Rust field is no reason to
   change what a log file says.

`DescentError::Blocked`'s field takes the same name rather than a different
one: it is the same folder, and it becomes `stopped_at` at every site the value
passes through.

### The sites

Rename the field and every binding, construction, pattern, doc comment and test
comment. Every site, by file:

**`coffret-usecase/src/descent_error.rs`** — the field (`:52`) and its doc
(`:47`, which must stop calling a local folder a component); the cross-reference
at `:64` (*"the reason `Blocked`'s component does"*); the `Redacted` doc at
`:121`–`:126`; the test comment at `:150` and the case
`a_blocked_place_says_it_was_blocked_and_never_which_component` (`:153`), whose
name asserts the very crossing this change removes — rename it to
`a_blocked_place_says_it_was_blocked_and_never_where_it_stopped`.

**`coffret-usecase/src/fetch/fetch_error.rs`** — the field (`:120`) and its doc
(`:112`); `from_descent`'s pattern and construction (`:296`–`:298`); both
`Display` arms (`:336`–`:346` and `:352`–`:361`); the `Redacted` arm (`:492`)
and the comment above it; the cases at `:610`–`:627`.

**`coffret-usecase/src/fetch/surfaced.rs`** — the field (`:75`) and its doc
(`:69`–`:74`); the `path()` arm at `:118` uses `{ path, .. }` and needs no
change, so check it rather than editing it.

**`coffret-usecase/src/fetch/select.rs`** — the pattern and the push at
`:92`–`:96`, which is where `DescentError::Blocked`'s value becomes
`Surfaced::UnreachablePlace`'s.

**`coffret-usecase/src/fetch/translate.rs`** — the `None` construction at
`:330`–`:333`.

**`coffret-usecase/src/destinations.rs`** — the capability's own contract prose:
`:101` says `Blocked` comes *"naming the component it stopped at"*, which is the
sentence that taught every implementation the wrong word. `:132` and
`local_place.rs:98` each say *"where a component on the way down is a symbolic
link"*: these are about a step of the descent rather than about the carried
value, and the repository already has the unambiguous phrase for it — *"A folder
on the way to a file is not one inside the mapped root"* (`descent_error.rs:83`).
Use that shape.

**`coffret-usecase/src/fetch/local_place.rs`** — the `# Errors` prose at `:98`
(see above). No code change.

**`coffret-usecase/src/in_memory_fs/state/folders.rs`** — four constructions:
`:59`–`:61`, `:72`, `:93`–`:95`, `:105`.

**`coffret-usecase/src/destinations_conformance/blocking.rs`** — the two
`match` arms at `:43` and `:81`, and the assertion message at `:45` (*"the
component the walk stopped at is what there is to look at"*).

**`coffret-usecase/src/destinations_conformance/vouching.rs`** — the
`matches!` at `:285`, which binds `ref path`.

**`coffret-local-fs/src/unix_destinations/mod.rs`** — the construction in
`refusal` at `:147`–`:149`. Its doc at `:137`–`:144` describes the errno
reading and needs no rename.

**`coffret-local-fs/tests/fetch_confinement.rs`** — the destructuring at
`:256`.

**`coffret-device/src/error.rs`** — `Error::descent`'s pattern and construction
at `:975`–`:978`, its doc at `:953`–`:972` (which says *"The folder the descent
stopped at travels with it"* — already the right word, so check it reads
correctly against the new field name), and the case at `:1092`–`:1095`.

**`coffret-device/src/findings.rs`** — the translation at `:140` and the
fixture at `:302`–`:305`.

**`coffret-device/src/finding_reason.rs`** — the field (`:67`) and its doc
(`:65`); the `Display` arm at `:105`–`:112`.

**`coffret-server/src/api_error/tests.rs`** — the constructions at `:83`,
`:90`, `:205`–`:207`, `:353` and `:360`. The local helper `fn component() ->
PathBuf` (`:316`) is part of the same crossing and is used by three different
refusals — `UnmaterializablePath` (`:360`), `RefusedRoot`'s `local_root`
(`:393`) and `RootRefused`'s `root` (`:401`) — so rename it to
`local_folder()`, which is true of all three, rather than to `stopped_at()`,
which is true of one. Leave
`a_reserved_component_writes_neither_the_path_nor_the_component` (`:412`)
alone: that case is about the Entry Path sense and its name is correct.

`coffret-server/src/noted.rs:151` and `api_error/mod.rs:205` match with `{ .. }`
and need no edit; `coffret-server/tests/routes.rs:2434` asserts the redacted
label, which does not change. Check them rather than editing them.

Nothing about behaviour changes: no variant is added or removed, every sentence
a person reads keeps saying what it said, and every redacted form stays
byte-identical. If a `Display` string or a `descent=` label differs after this
change, that is a mistake in the change and not an improvement to it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `FetchError::UnmaterializablePath` carries the local folder as
      `stopped_at`: `grep -rq 'stopped_at: Option<PathBuf>'
      backend/crates/domain/coffret-usecase/src/` matches and
      `grep -rq 'component: Option<PathBuf>' backend/crates/` matches nothing
      (both appended to `check_command`).
- [x] `Surfaced::UnreachablePlace`, `FindingReason::UnreachablePlace` and
      `DescentError::Blocked` carry theirs under the same name:
      `grep -rq 'stopped_at: PathBuf' backend/crates/apps/coffret-device/src/`
      matches, and `grep -rq 'component: PathBuf' backend/crates/` matches
      nothing — which also covers the two fixtures that build one
      (`coffret-device/src/findings.rs`, `coffret-server/src/api_error/tests.rs`).
- [x] No construction or pattern anywhere still spells the field `component`:
      `grep -rq 'component: None' backend/crates/`,
      `grep -rq 'component: Some(' backend/crates/`,
      `grep -rq 'UnreachablePlace { component' backend/crates/` and
      `grep -rq 'UnreachablePlace { path, component }' backend/crates/` each
      match nothing (all four appended to `check_command`).
- [x] `DescentError::Blocked`'s field is renamed at its definition and at every
      site that binds or builds it: `grep -rq 'Blocked { path' backend/crates/`,
      `grep -rq 'Blocked { ref path' backend/crates/` and
      `grep -rq 'path: at.to_path_buf()'
      backend/crates/gateway/coffret-local-fs/src/` each match nothing — the
      last being the gateway construction whose field sits on its own line
      (all three appended to `check_command`).
- [x] `coffret-server/src/api_error/tests.rs`'s local-folder helper no longer
      borrows the Entry Path word: `grep -rq 'fn component()'
      backend/crates/apps/coffret-server/src/` matches nothing (appended to
      `check_command`).
- [x] `descent_error.rs`'s case that pins the local folder out of the message
      and out of the event is renamed to match what it asserts:
      `grep -rq 'fn a_blocked_place_says_it_was_blocked_and_never_where_it_stopped'
      backend/crates/domain/coffret-usecase/src/` matches (appended to
      `check_command`), and `make check` runs it.
- [x] No doc comment, contract sentence or assertion message calls the local
      folder a component any more: over
      `backend/crates/domain/coffret-usecase/src/`,
      `grep -rq 'component it stopped at'`,
      `grep -rq 'component the walk stopped at'` and
      `grep -rq 'component the descent stopped at'` each match nothing — the
      capability contract at `destinations.rs:101`, the conformance assertion at
      `blocking.rs:45`, and the field doc plus test comment at
      `descent_error.rs:47` and `:150` (all three appended to `check_command`).
- [x] Every redacted form and every message is unchanged, proved by the cases
      that pin them still passing untouched under `make check`:
      `a_blocked_descent_keeps_its_shape_and_loses_both_paths`
      (`fetch_error.rs:609`),
      `a_blocked_place_says_it_was_blocked_and_never_where_it_stopped`
      (`descent_error.rs`), the `descent=blocked` / `descent=unspellable`
      assertions in `coffret-server/src/api_error/tests.rs` and
      `coffret-server/tests/routes.rs:2434`. Their expected strings must not be
      edited by this change.
- [x] The whole workspace builds and clippies clean
      (`make check` runs `cargo fmt --check`, `cargo build`, `cargo test` and
      `cargo clippy --all-targets -- -D warnings`), which is what proves no
      binding, pattern or `..` rest-pattern was left matching a field that no
      longer exists.

## Out of scope

- **`ReservedComponent`'s `component` field, in either enum.**
  `FetchError::ReservedComponent { component: String }` (`fetch_error.rs:154`)
  and `Surfaced::ReservedComponent` / `FindingReason::ReservedComponent` name a
  component of an Entry Path, which is what the Entry Path concept calls it
  (`docs/concepts/entry-path/README.md:7`). That name is correct and is the
  reason the other four have to stop borrowing it. Nothing about these changes.

- **The redacted labels `descent=blocked` and `descent=unspellable`**
  (`fetch_error.rs:493`). They name the descent's outcome, which is what a log
  file groups records by, and they are pinned in four places plus the worked
  example in `coffret-model/src/redacted.rs:28`. A field rename is not a reason
  to change what is written down.

- **The variant names `UnreachablePlace` and `UnmaterializablePath`.** The
  first crosses the wire by name (`coffret-server/src/api_error/mod.rs:492`,
  `frontend/packages/gateway/api/src/refusal.ts:90` and `:229`), and both name
  the state rather than the field. Unchanged.

- **`frontend/`.** Established above: no serde anywhere on the four types, and
  the only TypeScript mention of any of them is the variant name. No `.ts` or
  `.tsx` file is edited by this change.

- **`DescentError::Refused`'s `root` and `Io`'s `LocalIoError` path.** Both are
  local paths and both are already named for what they are — a mapped root and
  the path a call was made on. The crossing being fixed here is the one word
  used for two kinds of thing; these use two words for two things.

- **Earlier task files under `docs/tasks/2026/` that quote the old field name**
  (for instance
  `0911-1549-refuse-a-reserved-path-component-for-placement-and-report-a-refused-root.md:43`
  and `0904-1007-make-the-entry-path-shape-a-type-invariant.md:121`). They are
  the record of what the code was when those changes ran, and rewriting history
  to match today's names would make them lie about their own diffs.

- **Any behaviour change.** No variant added or removed, no message reworded, no
  redacted form altered, no new refusal. If the diff changes what a person reads
  or what a log records, it has gone beyond this change.

- **`backend/Cargo.lock`.** `make check` may touch it; a lock update in the diff
  is acceptable and needs no separate justification.
