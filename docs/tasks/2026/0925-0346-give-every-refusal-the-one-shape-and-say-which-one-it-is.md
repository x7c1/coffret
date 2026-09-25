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
branch: task/0925-0346-give-every-refusal-the-one-shape-and-say-which-one-it-is
created_at: 2026-09-25T03:46:49Z
updated_at: 2026-09-25T04:27:35Z
---

# fix(server): give every refusal the one JSON shape, and say which refusal it is

## Overview

The server promises that a refusal is one JSON shape — `error` (the kind),
`message`, `reason`, `surfaced` — and that the kind is a closed set a caller
branches on. `api_error/mod.rs` names the set on the `ApiError` struct doc and
`frontend/packages/gateway/api/src/refusal.ts` mirrors it as `RefusalKind`
plus the `KINDS` array the client checks a body against. Six places break that
promise or blur it: two answers that never reach the shape at all, two
refusals filed under a kind that says the wrong thing, one download with no
name, and one transport failure written into the log as a refusal. The wire
contract is fixed by tests rather than by prose (`tests/routes.rs`), so every
change below lands with the test that pins it.

The kind set grows by exactly two — `no_such_route` and `epoch` — and this
task is the one change that widens it. Add both to the struct doc's list, to
`RefusalKind` and to `KINDS`, in the same commit as the code that emits them.

### 1. An unknown route or method never reaches the JSON shape

`router.rs` registers eleven routes and nothing else. A path axum does not
know (`GET /api/entries`, the old spike's route) is answered by axum itself
with an empty-bodied 404, and a known path asked with a method it does not take
(`POST /api/file`) with an empty-bodied 405. Neither passes through any code
of this crate, so neither becomes an `ApiError`. The client's `refusalOf`
folds a body it cannot parse into `unrecognized` with the sentence "something
else replied", which is a lie here: it was this server.

Register both of axum 0.8's escape hatches — `Router::fallback` for the path
nobody registered and `Router::method_not_allowed_fallback` for the method
the registered path does not take — and answer from each with an `ApiError`
of kind `no_such_route`, at 404 and 405 respectively. One kind at two
statuses is the treatment `bad_request` already has at 400 and 413; say so in
the constructor's doc the way the struct doc says it for that one. The message
speaks about this server and nothing else — "this server answers nothing at
that path" / "... not by that method" — and repeats neither the path nor the
method: `unauthorized` already sets the rule that a refusal made before any
route reads the request does not echo the request.

Pin both in `tests/routes.rs`: the status, the kind, and that the body parses
as the shape.

### 2. `EpochActivated` reaches the page as "Storage did not answer"

`CommitError::EpochActivated` is a permanent state of the Library (spec: CP-5,
MR-2): a Master Key epoch was activated and this device must be re-enrolled
before it can commit. It is nothing a retry mends, so filing it with the
transient failures is the defect. There are four arms to count, not one:

- `from_error.rs::from_catch_up` puts it in the `server` (500) bucket beside
  `Index`, `EntryPathCollision` and the other programming-fault variants
- `from_sync`, `from_freeze` and `from_fetch` each fold the whole of
  `Commit(_)` into `storage` (502) "the Library's Storage did not answer",
  so an `EpochActivated` inside any of the three comes out under a sentence
  that is false of it — and so does every other non-Storage `CommitError`
  those arms swallow

Give it kind `epoch` through a constructor of its own, and make the three
`Commit(_)` arms delegate to the one classification `from_catch_up` already
holds (rename that function to say it classifies a commit, not a catch-up)
rather than restating a narrower one. Choose the status for `epoch` and write
the reason in the constructor doc: it is not the person's request that is wrong,
not the Storage, and not this process — it is this device's standing in the
Library. The message says what the person does next (re-enroll this device)
and never names the generation, which is Storage evidence and not a thing a
sentence needs (spec: EL-5).

Pin it: a route test that reaches the arm — the support module's store can be
put into the shape a catch-up meets, or the classification function can be
tested directly if the route cannot be driven there — asserts the kind and that
the message does not say Storage did not answer.

### 3. A listing that ran past its cap is filed as Storage not answering

`SyncError::ListingLimitReached { pages }` and `FreezeError::ListingLimitReached`
are both folded into `storage` "the Library's Storage did not answer"
(`from_error.rs` lines 135 and 183). Storage answered every page; what
happened is that the listing did not end within the pages this device reads.
The kind stays `storage` — the browser does not branch on it, and what is wrong
is still on the Storage side — but the sentence must say the listing ran past
the cap, not that nothing came back. Say the same sentence from both arms, and
pin it once from whichever route the support module can drive there (or from
the classification function).

### 4. A download nobody draws inline is saved as `file`

`routes/file.rs::served` sets `Content-Type` from `classify(path)` and nothing
else about the name. A format the explorer does not draw is served as
`application/octet-stream` (pinned by `a_file_no_browser_draws_is_served_as_bytes`),
and a browser that is handed that saves it as `file` with no extension.

Add `Content-Disposition` for that case, naming the file after the last
component of the Entry Path. The value is a header, so build it against header
injection rather than by formatting: the ASCII `filename=` fallback carries
only bytes a quoted-string admits (drop or replace CR, LF, `"` and anything
outside printable ASCII), and the real name travels in `filename*=UTF-8''`
percent-encoded per RFC 8187. Decide whether the formats the explorer does draw
get `inline` with the same name or no header at all, and say why in a comment.
Pin the header — one test with a plain name and one with a name that would
break a naive quoted-string (a quote, a non-ASCII character) — and pin that a
drawn format's answer is whatever you decided.

### 5. A transport failure mid-part is logged as a refusal of the part

`routes/upload/receive.rs` maps a `part.chunk()` failure to
`Refusal::Request(ApiError::multipart(cause))`. That constructor is for a
request whose multipart body could not be read as one — a 400 the person can
act on. A transport that broke while a chunk was in flight is not that: the
next `next_field()` in `upload/mod.rs` turns the same failure into the 400
that actually goes out, so the only effect of the arm is a per-part refusal
record in the log (`ApiError::record`) for a part nobody refused.

Read what `multer`'s error type distinguishes and tell the two apart: a body
that stopped arriving is not a refusal of the part. Decide what the arm hands
back so that the request ends the way it already ends and nothing is recorded
as refused that was not — the simplest honest shape is whatever makes the
outer loop's own handling the one place this is said. Pin that a transport
failure mid-part leaves no refusal record for the part (the logging test
double the crate already uses, if there is one; otherwise assert the
`Refusal` variant directly).

### 6. The route tests never travel the `locked` and 502 paths

`tests/routes.rs` pins `declined` five times and never pins `locked` (423)
or a `storage` (502) refusal end to end; the mapping is covered only by the
unit tests in `api_error/tests.rs`. Add one route test for each — a locked
server answering a route that needs the Master Key, and a Storage that does not
answer reaching `storage` — using the support module's existing doubles, and
extending `tests/support/mod.rs` only as far as those two need. Item 2 and
item 3 may share the Storage double this adds.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] an unregistered path and a registered path asked with the wrong method
      both answer the JSON shape with kind `no_such_route`, at 404 and 405,
      and a test pins each
- [x] `RefusalKind`, `KINDS` and the `ApiError` struct doc all name
      `no_such_route` and `epoch`, and a frontend test pins that both parse
      as themselves rather than as `unrecognized`
- [x] `EpochActivated` from any of the four arms answers kind `epoch` with a
      message that does not say Storage did not answer, and a test pins it
- [x] the non-Storage `CommitError` variants inside `Commit(_)` are classified
      the same way from sync, freeze, fetch and catch-up, and a test pins one
      of them from a wrapped arm
- [x] `ListingLimitReached` answers a message that says the listing ran past
      its cap, from both arms, and a test pins it
- [x] a download of a format the explorer does not draw carries
      `Content-Disposition` with a safe ASCII fallback and an RFC 8187 name,
      and tests pin a plain name and a hostile one
- [x] a transport failure mid-part leaves no refusal record for the part, and
      a test pins it
- [x] one route test reaches `locked` (423) and one reaches `storage` (502)

### Manual / on-hardware (verified by a human before merge)

- [ ] `make e2e-it` is green
- [ ] a file the explorer does not draw was downloaded from a real browser and
      saved under its own name

## Out of scope

- The shape of `coffret_device::Error` and `ApiError.cause: Option<String>`
  — a separate error-type change
- Splitting `tests/routes.rs` by flow
- Any kind beyond the two named here; a third would be its own contract change
- The vocabulary questions (`remote`, `present`, mapping) — a separate docs pass
