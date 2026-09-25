---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0925-0439-name-what-failed-on-this-device-and-carry-it-boxed
created_at: 2026-09-25T04:39:43Z
updated_at: 2026-09-25T07:18:24Z
---

# refactor(device): name what failed on this device, and carry the usecase verdicts boxed

## Overview

Five places where a failure on this device is typed by which operation was
running rather than by what went wrong, or where a variant carries a second
meaning its name does not admit. Each is a bounded change; together they are
the device-side half of the error-type work (the port side — carrying a Storage
failure across `coffret_usecase::Error` as a value — landed separately).

### 1. A catalog failure reaches a caller under three names

`coffret_device::Error` reports the catalog being unusable as `Index { cause }`
from `library_files.rs` and `mapping/mod.rs`, as `Fetch { cause: FetchError::Index }`
from every fetch flow, and as `LocalPathNotSettled { cause: FetchError::Index }`
from `local_path.rs` (lines 40 and 90) — where "where this path belongs was not
settled" is true but is not a verdict about the path at all; the catalog could
not be read. A caller that wants to say "the catalog could not be used" has to
match three shapes.

Make `Error::Index` the one name. Count the construction points first: every
`map_err` and `?` that lets a `FetchError::Index` become a `Fetch` or a
`LocalPathNotSettled`. Then decide the mechanism and write the reason where
the type is declared — either the device layer lifts the `Index` case out of
`FetchError` at each boundary before wrapping the rest, or `FetchError` stops
carrying the catalog failure as one of its own variants and the usecase's fetch
flows return it beside their own verdicts. The second is the honest shape (a
catalog that would not open is not something the fetch decided) but reaches
into `coffret-usecase`; take it only if the callers of `FetchError::Index` are
few enough to move in this change, and say which you chose. Either way
`LocalPathNotSettled` ends up carrying only the translation's own verdicts
(`UnmappedEntryPath`, `EntryNotCurrent`, `RefusedRoot`, …), and the server's
`api_error/from_error.rs` classifies an `Index` failure once.

### 2. Four usecase verdicts set the width of every device error

`Error::Sync`, `Freeze`, `Fetch` and `CatchUp` each hold their usecase enum by
value — 112 bytes apiece — so `coffret_device::Error` is 120 bytes and every
`Result` in the crate pays for the widest verdict. Box the four causes (the
gateway causes already are), keep `source()` / `redacted()` / `Display` as they
are, and add a test that asserts `size_of::<Error>()` against the number this
lands at, with a comment saying what the number is protecting — the test is
there so the next value-carrying variant is noticed, not to freeze the exact
figure forever.

### 3. `coffret-shell` still speaks `anyhow` in a public signature

`logging::start() -> anyhow::Result<()>` is public, and `passphrase.rs` builds
its failures with `bail!` / `.context(...)` and then folds the `anyhow::Error`
into `coffret_device::Error` through `not_given`. The workspace's own rule is
that each crate owns an `Error` / `Result` pair and does not put a type-erasing
library in a signature. Give `coffret-shell` an `error.rs` with the failures it
actually has — the prompt could not be read (carrying the `io::Error`), the two
Passphrases differed, the Passphrase was empty, standard input ended before one
was given, the log could not be started (carrying its cause) — express
`not_given` as a conversion from that type, and drop `anyhow` from the crate's
manifest. The sentences a person sees stay the same; pin them.

### 4. One header variant carries two different failures

`coffret_format::Error::InvalidChunkSize` is raised for a chunk size of zero
and for one past what this platform can address; its doc says the second "is
not a claim about the object", which is exactly why the two do not belong under
one name — a 64-bit reader opens what a 32-bit one refuses, and a caller that
wants to say "this build cannot read it" cannot tell that from "the header is
wrong". Read where `Truncated` is raised too: the same
platform meaning rides on it. Split the platform refusal into a variant of its
own (`UnaddressableOnThisBuild` or a name that says the same), raise it at the
exact points the address check fails, and leave the object-defect variants
accusing only the bytes. Every `match` on the format error that treats
`InvalidChunkSize` as an object defect is a construction point of this change;
count them across `coffret-format`, `coffret-usecase` and `coffret-device`.

### 5. A length Storage got wrong is treated as a verdict about the Library

`ChunkRunTruncated` and `ChunkRunOverrun` are raised by
`container_reader/chunk_run_reader.rs` when the bytes that arrived for a chunk
run are shorter or longer than the run the header declared. In
`fetch/container.rs::decode_into_place` the two error channels are two
different answers: the outer one is Storage's (a transfer that failed or came
up short, which the retry policy may attempt again) and the inner one is a
verdict about the Library, which no attempt would change. The two chunk-run
errors flow to the inner channel, although "the provider answered a ranged
read with a different length than it was asked for" is a Storage-side fact of
the same family as `LengthMismatch` / `LengthOverrun`, which the port already
marks retryable. Decide which channel they belong to — read how the reader
distinguishes a short body from a header that lies about its own lengths, since
only the first is Storage's doing — route the Storage-side case to the outer
channel, and pin it with a test that a short ranged read is attempted again
under the policy while a lying header is not.

### 6. Verify, then strike or fix

`Placement::stamp` / `parent` were also seen wrapping a domain
failure in a synthesised `io::Error` (`InvalidInput`) as `FetchError::Io`. A
search of `coffret-usecase` for `InvalidInput` finds nothing today. Confirm
against the tree; if it is gone, say so in your report and do nothing; if a
descendant of it survives under another kind, give `FetchError` the dedicated
variant the record asked for.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] a catalog failure reaches a device caller as `Error::Index` from the
      create, map, fetch and local-path entry points alike, and a test pins
      each; `LocalPathNotSettled` carries no `FetchError::Index`
- [x] `Error::Sync` / `Freeze` / `Fetch` / `CatchUp` hold their cause boxed,
      and a test asserts the size of `coffret_device::Error`
- [x] `coffret-shell` has its own error type, `anyhow` is gone from its
      manifest, and the Passphrase sentences a person sees are pinned
- [x] the platform refusal is a variant of its own in `coffret_format::Error`,
      raised where the address check fails, and no object-defect arm matches it
- [x] a chunk run that came up short from Storage goes to the outer channel
      and is attempted again under the policy; a header that lies about its
      lengths is not; a test pins both
- [x] no new `derive(PartialEq)` on any error type, and no `assert_eq!` on an
      error value

### Manual / on-hardware (verified by a human before merge)

- [ ] `make s3-store-it` is green

## Out of scope

- `ApiError.cause: Option<String>` — its struct doc gives the redaction reason
  for holding the rendering rather than the value, and this change keeps it
- Splitting any `error.rs` into a directory, and the two-meanings-of-`open`
  rename — the module-structure and naming change
- The port-side shape of `coffret_usecase::Error`, which the preceding change
  settled
