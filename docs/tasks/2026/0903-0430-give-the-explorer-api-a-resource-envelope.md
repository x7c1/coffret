---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rq 'from_stream\\|ReaderStream' backend/crates/apps/coffret-server/src && ! grep -rq 'DefaultBodyLimit::disable' backend/crates/apps/coffret-server/src"
assignee: null
branch: task/0903-0430-give-the-explorer-api-a-resource-envelope
created_at: 2026-09-03T04:30:00Z
updated_at: 2026-09-03T07:35:00Z
---

# feat(server): give the explorer API a resource envelope

## Overview

The Library's storage layer is deliberately format- and size-agnostic —
a five-gigabyte scan belongs in a Pack as much as a five-hundred-
kilobyte page does. The explorer server currently inherits that
open-endedness in two places where it should have its own, narrower
contract instead:

- `/api/upload` mounts with the body limit disabled outright. The parts
  stream to disk, but nothing bounds the request as a whole — total
  bytes, part count, concurrent uploads, or the staging space they
  consume while in flight. Disconnect cleanup trims what is left
  behind; it does not stop the disk from filling while the bytes are
  still arriving.
- `/api/file` reads the whole file into memory before answering. For
  the pages the explorer actually shows this is invisible; for a large
  Entry it makes process memory proportional to file size, on a route
  whose product explicitly stores files bigger than memory.

Give the server its own envelope, distinct from the Library's:

1. **Stream the file response.** `/api/file` hands the file to the
   response as a stream (a reader wrapped into the body), so process
   memory stays flat regardless of file size. Keep the existing
   no-Range decision — the reader is a browser rendering a page, not a
   media scrubber — and keep the refusal shapes for missing/unreadable
   files exactly as they are. A test pins that serving a file does not
   buffer it whole (assert on the mechanism — the body is built from a
   reader — not on RSS).
2. **Bound the upload request.** Replace the disabled body limit with
   explicit budgets, named constants beside the route with reasoned
   docs: a per-request total, a per-part total consistent with what a
   page of a scanned book can be, and a part count consistent with a
   drop of one book. Choose values generous enough that the freeze
   use case (hundreds of pages, tens to hundreds of megabytes each at
   the extreme) never meets them — the envelope refuses the absurd,
   not the ambitious. Exceeding a budget stops the request mid-stream
   with the existing refusal vocabulary, and whatever was staged is
   discarded — no final names appear (the existing scratch discipline
   should already guarantee this; pin it with a test).
3. **Bound the staging appetite.** A request that would exhaust the
   staging volume must fail cleanly rather than fill the disk: check
   available space against the declared/accumulated size at sensible
   points (before accepting a part is enough — this is a courtesy
   fence against accidents, not a quota system). Keep it simple; do
   not build accounting.
4. **One upload at a time is already the product's shape** — the
   explorer drops one book and waits. If the route does not already
   refuse concurrent freeze uploads, make the behavior explicit either
   way with a comment citing the freeze lifecycle; do not build a
   queue.
5. **Tests.** Router tests: an upload exceeding the total budget is
   refused mid-stream and leaves no final names and no scratch litter;
   a part over the per-part budget likewise; the file route streams
   (mechanism-level assertion) and still answers small files
   byte-identically. E2E: the existing journeys (which upload real
   books) pass unchanged — the proof the budgets clear the real use
   case.

Update the server crate doc: the resource envelope is now part of what
the server promises, separate from the Library's size-agnostic
storage contract.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `/api/file` streams: the response body is built from a reader,
      not a full in-memory buffer, with a mechanism-level test; small
      files are served byte-identically (the `from_stream|ReaderStream`
      grep gate pins the mechanism).
- [x] The upload route's body limit is no longer disabled: explicit,
      reasoned budgets replace it (the negative `DefaultBodyLimit`
      grep gate pins the removal), and an upload exceeding them is
      refused mid-stream — the refused part lands nothing (no final
      name, no scratch litter), while files fully landed before the
      refusal stay, as anything the user already handed over does —
      with tests.
- [x] A request that would outrun the staging volume's available space
      fails cleanly before filling the disk, with a test using a
      fabricated low-space answer.
- [x] The E2E journeys pass unchanged — the budgets clear the real
      one-book freeze use case.

## Out of scope

- HTTP Range support on `/api/file` — the no-Range decision stands.
- Rate limiting, request-duration timeouts, and per-client quotas.
- The Library's storage-layer contracts — Packs remain size-agnostic;
  this envelope is the server's own.
- The CLI paths (they do not pass through this API).
