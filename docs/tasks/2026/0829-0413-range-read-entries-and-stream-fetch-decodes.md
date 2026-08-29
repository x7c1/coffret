---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make s3-store-it && ! grep -rn 'into_bytes' backend/crates/domain/coffret-usecase/src/fetch/"
assignee: null
branch: task/0829-0413-range-read-entries-and-stream-fetch-decodes
created_at: 2026-08-29T04:13:47Z
updated_at: 2026-08-29T06:52:00Z
---

# feat(backend): range-read an Entry out of a Pack and stream fetch decodes

## Overview

`fetch_folders` (`backend/crates/domain/coffret-usecase/src/fetch/`) pulls
every Container whole into memory and decodes it in one buffer
(`fetch/container.rs` — `store.get(object, None)` + `into_bytes()` +
`coffret_format::decode`). That is affordable for the one-file image
Containers `sync` produces, but `freeze` now commits multi-GiB Packs, and a
reader that wants one page out of an unfetched book must not wait for — or
buffer — a 1–2 GiB object. This task extends the download path in two
steps, both consumers of the same new format-level capability:

1. **Chunk-level read API in `coffret-format`** — the read-side counterpart
   of the streaming `ContainerWriter`. The layout of a Container is fully
   determined by its plaintext header and meta section
   (`header.rs`, `layout.rs`): chunk `k`'s ciphertext occupies
   `Header::LEN + meta_len + k * (chunk_size + TAG_LEN)` onward, and each
   chunk authenticates independently (spec: FM-5, FM-6, FM-7, FM-8). Expose:
   parsing the header and decoding the meta section from a prefix of the
   object; the math from a plaintext byte range to the chunk run covering it
   and from a chunk run to its ciphertext byte range; and
   decrypt-plus-authenticate of an arbitrary chunk run, streaming plaintext
   out chunk by chunk without accumulating the object. The existing
   whole-buffer `decode()` may be reimplemented on top of it; its contract
   (every chunk authenticated before its bytes are exposed) is unchanged.

2. **`fetch` uses it, two ways.**
   - **Streaming whole-Container fetch.** `fetch/container.rs` stops
     collecting the ciphertext into memory: the object streams through the
     chunk decoder, wanted Entries are written to temporary files under the
     EP-11 placement discipline (unverified bytes never visible; BLAKE3 of
     each placed Entry compared against the catalog before rename), and the
     BLAKE3-256 of the ciphertext is computed over the stream as it passes
     and compared against what the Journal record carried (spec: FM-15,
     CP-11) before any placement becomes visible. Memory stays bounded by
     chunk-sized buffers however large the Pack is.
   - **Partial fetch of one Entry.** A new public entry point in
     `coffret_usecase::fetch` fetches a single wanted Entry out of a Pack
     without pulling the rest: the Index already holds the Entry's Container
     and its offset/size inside the plaintext stream (`entries` table,
     spec: FM-15), so the flow rounds the Entry's plaintext range to chunk
     boundaries, issues one ranged `ObjectStore::get`
     (both adapters already honor `Range<u64>`), decrypts and authenticates
     exactly the covered chunks, and places the Entry under the same EP-11
     discipline — temporary file, the Entry's own mtime, rename, then
     `mark_present`. A range read cannot check the whole-ciphertext hash;
     per-chunk AEAD authentication is the integrity gate for the bytes that
     arrive (spec: FM-5, FM-8), and the Entry's plaintext BLAKE3 against the
     catalog remains the final gate before the file becomes visible. Per
     PK-16 this is an optimization inside a Container fetch, not a new fetch
     unit — completing the rest of the Container later (background fill) is
     the viewer's concern and out of scope here.

The fetch module's rustdoc (`fetch/mod.rs`) currently lists chunk-to-disk
decode and range-read prefetch under "deliberately not here"; update that
list to match what moves in. Wire format, spec register, and the TS second
implementation are untouched — this is read-side only. Follow the error
conventions in `CLAUDE.md` (typed causes, no stringified errors) and keep
retry behavior on the existing `RetryPolicy` contract (each attempt opens a
fresh stream).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-format` exposes a chunk-run read API with unit tests
      covering: an Entry contained in a single chunk, an Entry spanning a
      chunk boundary, and an Entry ending in the final chunk (padding tail
      and final-chunk domain handled); a tampered chunk inside the requested
      run fails with a typed error and yields no plaintext.
- [x] `fetch_conformance` gains a partial-fetch case: from a committed
      multi-entry Pack, fetching one Entry issues only ranged `get` calls
      (observed via the counting store) whose total requested bytes are
      strictly less than the object's length, and the placed file's content
      equals the original — with the Entry marked present afterwards.
- [x] `fetch_conformance` covers the refusal paths of the partial fetch: a
      mangled chunk inside the requested range fails with a typed error and
      nothing becomes visible at the destination; an Entry whose plaintext
      hash does not match the catalog is refused before rename.
- [x] The existing whole-fetch conformance cases (round trip, integrity,
      mangling, conflicts, keyring, scope) pass unchanged over the streaming
      implementation, including the ciphertext-hash refusal
      (`mangling_store`) with nothing visible on failure.
- [x] The fetch module no longer buffers whole objects:
      `grep -rn 'into_bytes' backend/crates/domain/coffret-usecase/src/fetch/`
      returns no matches (this grep is appended to `check_command`; it
      matches `fetch/container.rs` today, so the gate flips with the
      change).
- [x] All of the above hold against MinIO as well (`make s3-store-it` runs
      the fetch conformance suite there).

## Out of scope

- Resuming an interrupted fetch from verified bytes (HTTP Range resume) and
  background fill of the rest of an opened Pack — the viewer's prefetch
  machinery.
- The viewer server connection, thumbnails, and derived Entries.
- Propagating updates and deletions into Packs (PK-9..PK-12).
- A persistent download-cache policy beyond the placed files themselves.
- Keyring repair (KL-11, KL-13) and S3 multipart upload.
- Real-provider (Google Drive) execution of the new cases — deferred to the
  on-device verification pass.
