---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rq 'cause: getrandom::Error' backend/crates/apps/coffret-device/src/ && ! grep -rq 'detail: String' backend/crates/apps/coffret-device/src/ && grep -rq 'Device::ServerKeyNotDrawn: ' backend/crates/apps/coffret-device/src/ && grep -rq 'fn a_server_key_that_could_not_be_drawn_carries_what_the_source_reported' backend/crates/apps/coffret-device/src/ && grep -rq 'cause: argon2::Error' backend/crates/domain/coffret-format/src/ && ! grep -rq 'detail: error.to_string()' backend/crates/domain/coffret-format/src/stored_master_key/ && grep -rq 'fn an_argon2id_refusal_carries_what_the_implementation_reported' backend/crates/domain/coffret-format/src/ && ! grep -rq 'structure it could not read' backend/crates/domain/coffret-format/src/ && ! grep -rq 'foreign cause the blanket arm renders' backend/crates/domain/coffret-format/src/"
assignee: null
branch: task/0912-0436-carry-the-entropy-and-argon2id-failures-as-values
created_at: 2026-09-12T04:36:17Z
updated_at: 2026-09-12T05:24:49Z
---

# refactor(backend): carry the entropy and Argon2id failures as values, not strings

## Overview

Three variants across two crates still flatten a failure that arrived as a Rust
value into a `String`, and one doc paragraph makes a claim about `detail`
strings that is no longer true of the enum it is written on. The rule they are
measured against is already written down in this repository, in the Drive
gateway's own error vocabulary
(`backend/crates/gateway/google-drive-store/src/error.rs:22`): *"A failure this
layer observed as a Rust error travels as that error: the value goes in a
`cause` … What a remote reported — a status, a body, a message from the token
endpoint — is text where it arrived as text, and stays text."*
`coffret_format::Error::EntropyUnavailable`
(`backend/crates/domain/coffret-format/src/error.rs:647`) is the worked example
of the shape: it holds `cause: getrandom::Error`, renders it in `Display`
(`error.rs:966`), returns it from `source()` (`error.rs:978`), and carries a
`Redacted` paragraph saying why a foreign cause's own rendering is still
log-safe (`error.rs:1000`, spec: EL-3, EL-4). The three variants below are the
same situation and do not follow it.

**`coffret_device::Error::ServerKeyNotDrawn` flattens the same entropy error.**
The variant is `ServerKeyNotDrawn { detail: String }`
(`backend/crates/apps/coffret-device/src/error.rs:121`), and its one
construction site is `getrandom::fill(&mut bytes).map_err(|cause|
Error::ServerKeyNotDrawn { detail: cause.to_string() })`
(`backend/crates/apps/coffret-device/src/server_key.rs:61`) — the value from the
same `getrandom` call the format crate already carries whole. Nothing else in
this crate does that: `KeyMaterial { cause: coffret_format::Error }`
(`error.rs:112`) and `RootMarkerNotDrawn { root, cause: coffret_format::Error }`
(`error.rs:272`) are the entropy refusals that go through the format layer, and
both carry the value. The enum's own head doc already states the rule it breaks
(`error.rs:25`): *"What a lower layer reported travels as the typed `cause` it
reported, so a caller printing the chain sees the format crate's, the Index's,
or the gateway's own answer rather than a copy of it made here."* A caller
holding this variant today cannot ask which kind the entropy source named, and
`source()` puts it in the `None` group (`error.rs:757`) so the chain stops at a
sentence this crate composed.

**`coffret_format::Error::InvalidArgon2Params` and `PassphraseDerivationFailed`
flatten an `argon2::Error`.** Both are `{ detail: String }`
(`backend/crates/domain/coffret-format/src/error.rs:639` and `:644`), and both
are built in `stored_master_key/argon2_params.rs:76` and `:83` with
`error.to_string()` over an `argon2::Error` — a 16-variant enum whose value says
exactly which cost parameter Argon2id refused (`MemoryTooLittle`,
`ThreadsTooMany`, `TimeTooSmall`, …). That is the structured value the rule is
about, and it is thrown away at the `map_err`. `source()` returns nothing for
either variant (`error.rs:979`, the `_ => None` arm).

**The `Redacted::redacted` doc paragraph on `coffret_format::Error` says
something untrue.** `error.rs:993` reads *"The few `detail` strings are a CBOR
decoder's account of a structure it could not read, and carry no value out of
it."* That covers neither the Argon2id pair nor the strings this crate composes
itself — `"{} bytes follow the payload map"` (`control/cbor/mod.rs:53`), the
field-and-bound sentences in `control/cbor/fields.rs`, and
`meta/encode.rs:71`'s encode failures. `error.rs:1000` then says *"The one
foreign cause the blanket arm renders"*, which stops being one the moment the
Argon2id causes are carried. Both sentences have to be rewritten as part of this
change, not after it.

### What to change

**1. `coffret_device::Error::ServerKeyNotDrawn` carries the value.** Replace
`detail: String` with `cause: getrandom::Error` at
`coffret-device/src/error.rs:121`, keeping a field doc in the voice of
`EntropyUnavailable`'s (`coffret-format/src/error.rs:648`). Render it in the
`Display` arm (`error.rs:564`) so the sentence a person reads still ends with
what the source said. Move the variant out of `source()`'s `None` group
(`error.rs:757`) into an arm returning `Some(cause)`. In `redacted()`, replace
`"Device::ServerKeyNotDrawn".to_owned()` (`error.rs:851`) with a rendering that
carries the cause — `format!("Device::ServerKeyNotDrawn: {cause}")` — and say
in the `redacted()` doc why that is log-safe, citing EL-3 and EL-4 and the
reasoning `coffret-format/src/error.rs:1001` already gives for the same value:
`getrandom` prints either a sentence of its own about this machine's random
source or the operating system's message for the errno it was given, and
neither names a path, a filename, or anything anybody chose. Then hand the value
over at the construction site: `server_key.rs:61` becomes
`map_err(|cause| Error::ServerKeyNotDrawn { cause })`.

**2. The Argon2id pair carries the value.** Replace `detail: String` with
`cause: argon2::Error` on both `InvalidArgon2Params`
(`coffret-format/src/error.rs:639`) and `PassphraseDerivationFailed`
(`:644`); render it in the two `Display` arms (`error.rs:960` and `:963`); and
add a `source()` arm for both beside `EntropyUnavailable`'s (`error.rs:978`).
`argon2_params.rs:76` and `:83` then hand the value over rather than
`error.to_string()`.

`argon2::Error` implements `std::error::Error` only under argon2's `std`
feature, which the workspace does not enable today: `backend/Cargo.toml:29`
reads `argon2 = { version = "0.5", default-features = false, features =
["alloc"] }`. Add `"std"` to that list, with a comment saying what it is for —
so the Argon2id refusal can sit in a `source()` chain. This pulls in no new
crate: argon2's `std` expands to `["alloc", "password-hash/std"]`, and
`password-hash 0.5.0` is already in `backend/Cargo.lock:2355` with `base64ct`,
`rand_core` and `subtle`, all of whose `std` features only switch behaviour in
crates already built. `argon2::Error` is `Copy + Clone`, so
`coffret_format::Error`'s `derive(Debug, Clone)` (`error.rs:31`) still holds,
exactly as it does with `getrandom::Error` today.

`backend/Cargo.lock` does not change: a lock file records resolved versions,
not which features a dependency was asked for, and this feature resolves to
crates the lock already pins.

**3. The two `Redacted` doc sentences.** Rewrite `error.rs:993`'s claim so it
describes what the `detail` strings actually are: partly an account a CBOR
reader gave of bytes that are not the shape a schema spells, partly sentences
this crate composes about its own arithmetic — and, either way, nothing lifted
out of a payload, which is the property that makes them log-safe. Rewrite
`error.rs:1000`'s *"The one foreign cause the blanket arm renders"* so it covers
all three: `EntropyUnavailable`'s `getrandom::Error` and the Argon2id pair's
`argon2::Error`. For the Argon2id half, the reason it holds is that
`argon2::Error`'s `Display` is a fixed sentence about a cost parameter
(`"memory cost is too small"`, `"not enough threads"`, …) chosen from a closed
set at compile time: no Passphrase, no salt, no path, and nothing a person
typed can reach it (spec: EL-3, EL-4).

While there: the enum head doc's *"The one value that still reaches a message is
whatever ciborium quotes in its own text"* (`error.rs:28`) is a claim about
values a **payload** carried, and stays true — but it now sits next to two
foreign causes whose renderings also reach a message, so tighten it to name the
payload explicitly rather than leaving it readable as a claim about every value
in the enum.

**4. A test for each carried cause, in the shape the existing one has.**
`coffret-format/src/error.rs`'s test
`an_entropy_failure_carries_what_the_source_reported` (`error.rs:1054`) is the
model: it downcasts `source()` to prove the value and not a rendering is under
the chain, and it composes the expected `redacted()` string from the cause's own
rendering rather than writing the upstream sentence out, so a reworded upstream
message is not a failure of this layer. Add, in the same module,
`an_argon2id_refusal_carries_what_the_implementation_reported`, over
`Error::InvalidArgon2Params { cause: argon2::Error::MemoryTooLittle }`,
asserting the same three things. Add, in `coffret-device/src/error.rs`'s test
module, `a_server_key_that_could_not_be_drawn_carries_what_the_source_reported`,
over `Error::ServerKeyNotDrawn { cause: getrandom::Error::UNSUPPORTED }`,
asserting that `source()` downcasts to `getrandom::Error` and that `redacted()`
is composed from the cause's own rendering. Assert with `matches!` or a `match`
on the variant where a whole error value would otherwise be compared — neither
enum derives `PartialEq` and neither may gain it.

Also extend the existing case
`parameters_argon2id_refuses_are_reported_as_such`
(`argon2_params.rs:155`) so it asserts the carried cause rather than only the
variant: `Argon2Params::new(0, 1, 1)` is refused for memory cost, which is a
fact the variant alone no longer has to leave unsaid.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret_device::Error::ServerKeyNotDrawn` carries the entropy failure as
      a value: `grep -rq 'cause: getrandom::Error'
      backend/crates/apps/coffret-device/src/` matches and
      `grep -rq 'detail: String' backend/crates/apps/coffret-device/src/`
      matches nothing — the crate's only `detail` field was this variant's.
      Both greps are appended to `check_command`.
- [x] `server_key.rs` hands the `getrandom` value to the variant instead of
      `cause.to_string()`, and the whole backend still builds and clippies
      clean under `make check` — which is what proves the `Display`, `source()`
      and `redacted()` arms were all updated together rather than one of them
      being left matching on a field that no longer exists.
- [x] `coffret_device::Error::redacted()` renders the entropy cause for that
      variant rather than stopping at the variant name:
      `grep -rq 'Device::ServerKeyNotDrawn: '
      backend/crates/apps/coffret-device/src/` matches (appended to
      `check_command`).
- [x] A case named
      `a_server_key_that_could_not_be_drawn_carries_what_the_source_reported`
      exists under `backend/crates/apps/coffret-device/src/` and passes,
      asserting that `source()` downcasts to `getrandom::Error` and that the
      redacted form is composed from the cause's own rendering (the name is
      grepped in `check_command`; `make check` runs it).
- [x] `coffret_format::Error::InvalidArgon2Params` and
      `PassphraseDerivationFailed` carry `argon2::Error`:
      `grep -rq 'cause: argon2::Error'
      backend/crates/domain/coffret-format/src/` matches, and
      `grep -rq 'detail: error.to_string()'
      backend/crates/domain/coffret-format/src/stored_master_key/` matches
      nothing (both appended to `check_command`).
- [x] A case named
      `an_argon2id_refusal_carries_what_the_implementation_reported` exists
      under `backend/crates/domain/coffret-format/src/` and passes, asserting
      the value is under `source()` and that the redacted form is composed from
      the cause's own rendering (the name is grepped in `check_command`).
- [x] `Redacted::redacted`'s doc on `coffret_format::Error` no longer claims the
      `detail` strings are only a CBOR reader's account of a structure it could
      not read, and no longer claims the blanket arm renders a single foreign
      cause: `grep -rq 'structure it could not read'` and
      `grep -rq 'foreign cause the blanket arm renders'` over
      `backend/crates/domain/coffret-format/src/` each match nothing (both
      appended to `check_command`).
- [x] Neither `coffret_format::Error` nor `coffret_device::Error` gains
      `derive(PartialEq)`, and the new cases assert on variants rather than
      comparing whole error values — enforced by `make check` (`clippy
      --all-targets -- -D warnings`) plus the diff, since adding the derive
      would be the only way an `assert_eq!` on an error value could compile.

## Out of scope

- **The CBOR-derived `detail` strings stay strings.** `MalformedMeta`,
  `MetaEncodeFailed`, `MalformedControlPayload`, `ControlPayloadEncodeFailed`,
  `MalformedJournalRecord`, `MalformedIndexSnapshot` and
  `MalformedKeyringPayload` (`coffret-format/src/error.rs:66`, `:71`, `:316`,
  `:336`, `:347`, `:369`, `:379`) each keep `detail: String`, and the
  constructors that build them (`control/cbor/mod.rs:78`,
  `control/payload/mod.rs:58` and `:78`, `meta/mod.rs:75`,
  `meta/encode.rs:71`, `control/journal_record/decode.rs:148`,
  `control/index_snapshot/decode.rs:203`, `control/keyring/decode.rs:81`,
  `control/cbor/fields.rs:198`) are left as they are. Three reasons, and the
  third is the decisive one.

  The value was never discarded: `malformed_cbor`
  (`coffret-format/src/malformed_cbor.rs:19`) *destructures*
  `ciborium::de::Error::Semantic(_, message)` and passes the message through
  because ciborium's `Display` is its `Debug` spelling — a `to_string()` of the
  whole error would reach a caller as `Semantic(None, "…")` with the useful
  message quoted inside it. Two cases already pin that this does not happen
  (`control/journal_record/rejection_tests.rs:252` asserts the detail carries no
  `Custom(`, `:317` no `Semantic(`). Putting the typed error in the cause chain
  would reintroduce exactly the rendering those cases exist to prevent.

  The type cannot be carried anyway. `ciborium::de::Error<T>` is generic over
  the reader's error and derives `Debug` alone — no `Clone` — while
  `coffret_format::Error` derives `Clone` (`error.rs:31`), which the crate's
  callers rely on.

  And the variants have mixed provenance, which is what settles it: the same
  `detail` field is also built from sentences this crate composes with no
  upstream value behind it at all — `"{} bytes follow the payload map"`
  (`control/cbor/mod.rs:53`), the field-name-and-bound sentences in
  `control/cbor/fields.rs:198`, and `describe`'s account of the CBOR item found
  in a field's place (`control/cbor/mod.rs:103`). A variant whose content is a
  message the code itself composes is not a flattened cause, and splitting one
  verdict into
  a carried-cause variant and a composed-message variant would give one
  refusal two spellings for no caller's benefit. The `Redacted` doc rewritten
  above is where this answer lives in the code, so the next reader of the enum
  finds it there.

- **`TransportError`'s `detail` strings**
  (`backend/crates/gateway/google-drive-store/src/http/transport_error.rs:14`,
  `:19`, `:24`, built at `http/reqwest_transport.rs:86`). Same mixed
  provenance, and the crate's own head doc already draws the line it is on:
  what a *remote* reported is text where it arrived as text. Not touched here.

- **`FetchError`'s two senses of `component`.**
  `UnmaterializablePath { component: Option<PathBuf> }` is the local folder a
  descent stopped at, while `ReservedComponent { component: String }` is a
  component of an Entry Path — and `Surfaced::UnreachablePlace`
  (`coffret-usecase/src/fetch/surfaced.rs:75`),
  `FindingReason::UnreachablePlace`
  (`coffret-device/src/finding_reason.rs:67`) and `DescentError::Blocked`'s
  `path` (`coffret-usecase/src/descent_error.rs:52`) cross the same way. It is a
  public field rename reaching about fifteen files across four crates,
  including two conformance suites, and it shares only
  `coffret-device/src/error.rs` with this change. Its own change, and one whose
  whole diff should be that rename.

- **`MalformedMarker::NotText` discarding the `Utf8Error`**
  (`coffret-usecase/src/root_marker.rs:103`). Already settled and deliberately
  not carried: `defect()` (`root_marker.rs:144`) decides that a marker file's
  content never reaches an event and only which of the three ways it failed
  does, and a UTF-8 byte offset into a file out of somebody's folder refuses
  nothing a caller acts on and distinguishes no failure `defect()` does not
  already name (spec: EL-1, EL-3, EL-4). Not reopened here.

- **Any behavioural change.** Every refusal in this change keeps the variant it
  had, the sentence a person reads keeps saying the same thing, and no new
  refusal is introduced. What changes is what a caller can inspect and what a
  printed chain reaches.
