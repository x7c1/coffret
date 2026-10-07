---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1007-1800-say-how-far-a-pack-has-gone-while-it-is-sent-to-storage
created_at: 2026-10-07T08:35:11Z
updated_at: 2026-10-07T09:02:35Z
---

# feat(explorer): say how far a Pack has gone while it is sent to Storage

## Overview

Dropping a book of a few hundred pages (192 images, about 60 MB) into a folder
made with "new folder" shows "adding 200 files to …" for an instant — the upload
from the page to the local server is fast — and then
"packing test-03 — sending 1/1…" for a long while, until it turns into
"packed 200 files of … into 1 Pack". The long while is the one Pack being sent
to Google Drive. Progress is counted in units (`Step { done, total }` in
`coffret-usecase/src/progress.rs`), and a book is one Pack, so the count reads
1/1 the whole time and a person cannot tell a slow upload from a stuck one.

**1. A byte count for the unit being sent.** `progress.rs` says a step is
"never a byte count or a share of one file". That holds for phases made of many
small units; it fails for the uploading phase of a freeze, whose unit can be a
Pack of tens of megabytes. Keep the unit count, and let a step of
`Phase::Uploading` also carry how many bytes of the unit in flight have been
sent out of how many — or the bytes of the whole phase, if that is simpler and
as true; choose and say why in the doc comment, which is rewritten to state the
new rule rather than the old one. Report it at most a few times a second.

**2. Count what Storage has taken.** Storage writes take the body as a stream
(`ObjectStore::put(name, body: ByteStream)`). Wrap the Pack's stream in the use
case so the bytes the store has pulled are counted, without changing the port.
That count only means "sent" if the store sends as it pulls. Check each store
(`google_drive_store`, `s3_store`, the local filesystem destination). A store
that reads the whole body into memory before sending it — a multipart Drive
upload that builds the request in memory, say — is a bug, not a limit to report
around: memory then grows with the size of a book, and the design is that
bodies are streamed both ways. Fix such a store so it sends the body as it
pulls the stream (for Drive, a resumable upload sent in chunks, or a streamed
multipart body), and keep the count measured from what the store pulls, which
is then true for every store. The Drive store is the one that matters: it is
where the wait is.

**3. Through the work route to the explorer.** The server's work answer
(`coffret-server/src/routes/work/step_dto.rs` and the freeze progress it
reads) carries the byte count, and so does the TypeScript API contract in
`frontend/packages/gateway/api`. The explorer's packing line shows it beside
the count, in the same units and wording the adding line already uses
(`fill.ts`, `addingLine`: "23.0 MB of 58.0 MB"), for example
"packing test-03 — sending 1/1 — 23.0 MB of 60.0 MB…". A sync's uploading
phase may show bytes too if it comes for free; it is not required.

Out of scope: the command line's progress output (it must still build and
pass its tests), a byte count for fetching, and changing how a book is cut
into Packs.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] A step of the uploading phase can carry a byte count, and `progress.rs` documents when a step carries one and why
- [x] The freeze reports bytes sent for the Pack in flight, counted from what the store takes, with a unit test that a slow store's put is seen part way (a test store that pulls the stream in pieces)
- [x] No store reads a whole body into memory before sending it; where one did, it is fixed, with a test that a body larger than one chunk is pulled in pieces while the request is being sent, where the store's tests can do so without a real service
- [x] The work route and the API contract carry the byte count, with route or DTO tests
- [x] The explorer's packing line shows "N MB of M MB" during the upload, with unit tests for the wording in `fill.test.ts`
- [x] `make check` passes

### Before merge (verified outside the check command)
- [x] For each store, the PR says whether it sent the body as it pulled it before this change and what was changed where it did not, from reading its put; for Google Drive this is checked against the request it builds — an agent reads the code and reports
- [ ] Needs a person: under `make desktop-dev`, dropping a book of tens of megabytes into a new folder of the development Library shows the packing line's megabytes rising while it is sent to Drive
