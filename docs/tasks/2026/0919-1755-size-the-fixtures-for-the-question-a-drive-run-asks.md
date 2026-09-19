---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'photo_size' backend/crates/apps/coffret-fixtures/src/main.rs && grep -q 'page_size' backend/crates/apps/coffret-fixtures/src/main.rs && grep -q 'photo-size' scripts/drive-round-trip-it.sh && grep -q 'page-size' scripts/drive-round-trip-it.sh && ! grep -Eq 'COFFRET_ROUND_TRIP_PHOTOS:-12' scripts/drive-round-trip-it.sh && bash -n scripts/drive-round-trip-it.sh && grep -q 'PHOTO_SIZE' Makefile && grep -q 'PAGE_SIZE' Makefile"
assignee: null
branch: task/0919-1755-size-the-fixtures-for-the-question-a-drive-run-asks
created_at: 2026-09-19T17:55:16Z
updated_at: 2026-09-19T19:50:00Z
---

# test(drive): size the fixtures for the question a Drive run asks

## Overview

`make drive-round-trip-it` (`scripts/drive-round-trip-it.sh`) takes a
folder of generated JPEGs into a Library on real Google Drive and back
out. Its question is whether the round trip holds, and the answer does
not depend on how many bytes go round — but the cost of asking does. A
run today generates 12 photos at 1600×1200 and 3 pages at 1200×1800
(`PHOTOS` / `PAGES` at `scripts/drive-round-trip-it.sh:79-80`), which is
15 files and about 1.7 MB per run, and every one of those files stays on
the account and on this disk because the target reuses its Library.
Each Drive put takes seconds, so the file count decides how long a run
waits, and the byte count decides how fast the account fills. Both are
far larger than the question needs.

The generator, `coffret-fixtures`
(`backend/crates/apps/coffret-fixtures/src/main.rs`), was written for the
viewer benchmark, where camera-sized images are the point, and it hard
codes the two image sizes in `photo()` and `page()`. It has no way to
ask for small ones. Give it one, and make the round trip use it:

1. **Dimensions as arguments.** Add `--photo-size WxH` and
   `--page-size WxH` to `Args`, parsed from `WIDTHxHEIGHT` with a clear
   error on anything else, defaulting to the sizes the two functions
   hard code today (1600×1200 and 1200×1800) so `make fixtures` and the
   benchmark are unchanged. `photo()` and `page()` take the size instead
   of naming it; the block scatter in `photo()` and the line runs in
   `page()` must still produce a valid image at small sizes (say 64×48),
   so their margins and block sizes have to scale or clamp rather than
   overflow the canvas. Keep the output deterministic per index and
   size. Update the crate doc comment, which currently says
   "camera-sized JPEGs" as if that were the only thing it makes.
2. **The Makefile passes them through.** `make fixtures` takes
   `PHOTO_SIZE` and `PAGE_SIZE` beside `OUT` / `PHOTOS` / `PAGES`, with
   defaults that leave the current behaviour alone. Document them in the
   `## fixtures:` help line the way the existing variables are.
3. **The round trip asks for little.** Change the defaults in
   `scripts/drive-round-trip-it.sh` so a run generates a handful of
   files totalling in the hundreds of kilobytes at most, and say in the
   comment above them why that size is enough. Something like 3 photos
   and 2 pages at a few hundred pixels a side; measure and pick sizes
   that keep the whole batch under about 150 KB. The overrides
   (`COFFRET_ROUND_TRIP_PHOTOS` / `_PAGES`) stay, and gain
   `COFFRET_ROUND_TRIP_PHOTO_SIZE` / `_PAGE_SIZE` beside them. The
   script's own header comment and the `## drive-round-trip-it:` block
   in the Makefile say how much a run adds; bring both in line with the
   new numbers. Keep the fixture layout (`album-000/`, `book-000/`) as
   it is: the script's deletion step picks the first file by name from
   `album-000` and must go on finding one.
4. **A unit test for the parsing and the small sizes.** A test in the
   fixtures crate that parses `WxH` (and rejects `W`, `WxHx1`, `0x0`),
   and one that renders a photo and a page at a small size and checks
   the dimensions and that the buffer was written. The crate has no
   tests today; put them under `#[cfg(test)]` in `main.rs` or split the
   rendering into a module if that reads better.

Manual verification runs the target once against real Drive. The
consents are already answered on the machine that runs it, so the run is
unattended; what it should show is the new file count and a batch that
is a fraction of the old size.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-fixtures` accepts `--photo-size WxH` and `--page-size WxH`, defaults to the sizes it hard codes today, and rejects a malformed size with an error that names the expected form
- [x] `make fixtures` accepts `PHOTO_SIZE` and `PAGE_SIZE` and leaves its output unchanged when neither is given
- [x] `scripts/drive-round-trip-it.sh` generates a batch of at most a handful of files in the hundreds of kilobytes by default, passes the sizes through to the generator, and its comments and the Makefile's `## drive-round-trip-it:` block describe the new size
- [x] Unit tests cover size parsing and rendering at a small size

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-round-trip-it` runs green against real Drive with the new defaults, and the batch it generates under `.tmp/drive-round-trip/main/runs/<stamp>/` is under about 150 KB

## Out of scope

- `scripts/drive-index-layout-it.sh`, which writes its own three text files and does not use the generator
- Cleaning up what earlier runs left on the account or on disk
- Any change to what the viewer benchmark (`make fixtures` with its defaults) produces
