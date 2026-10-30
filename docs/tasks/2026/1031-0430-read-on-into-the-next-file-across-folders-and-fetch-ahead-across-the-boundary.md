---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1031-0430-read-on-into-the-next-file-across-folders-and-fetch-ahead-across-the-boundary
created_at: 2026-10-30T19:30:15Z
updated_at: 2026-10-30T21:10:57Z
---

# feat: read on from the end of one folder into the next file in the reader, and fetch ahead across the boundary

## Overview

With the reader open, ↑ and ↓ move the tree's focus to the row above or below,
and the reader follows it. At the last file of a folder, ↓ lands on the next
folder's row and the reader shows that folder's cover; someone reading a
series volume by volume has to step into the next volume by hand. And the
reader fetches ahead only inside one folder (three pages either side, forward
first, `prefetch.ts` / `sheet.ts`), so the first page of the next volume waits
for its Container's header, meta section and parcel — three round trips to
Storage when the volume was never listed.

**Keys with the reader open.**

1. ↓ moves to the next *file* in Entry Path order, at any depth: from the last
   page of a volume to the first page of the next volume, or to the next file
   of the parent folder. Folder rows are passed over.
2. ↑ the same backwards: from the first file of a folder to the last file of
   the previous one.
3. Only while the reader is open. With it closed, ↑ ↓ move between the rows on
   show as now.
4. A folder the move lands in that is closed is opened and the focused row
   scrolled into view. Folders read through are left open.
5. A file the reader cannot show is not skipped: the focus stops on it and the
   sheet says the format is not supported, as inside a folder today. The
   counter and the details pane's Items stay consistent.
6. At the last file of the Library ↓ does nothing, and at the first ↑ does
   nothing; it does not wrap.
7. The tree holds listings only of the folders it has opened, so it cannot know
   the next file inside a closed folder. Add a route to the loopback API that
   answers, for an Entry Path, the next and the previous file in Entry Path
   order across the whole Library (from the catalog, no Storage round trip),
   and use it. Add its rule beside the listing route's in `docs/spec/`
   (loopback access) and its typed call to the gateway
   (`frontend/packages/gateway/api/`).

**Fetching ahead across the boundary.**

8. When the focus comes within 10 files of the end of a folder (in reading
   order) and the next file lies in another folder, fetch ahead for that
   folder: list it (so the server reads its Containers' fronts ahead,
   `fronts_ahead.rs`) and read its first few pages. Ten, not three: three
   pages are read before the fetch ahead can finish. Inside a folder the
   prefetch stays at three either side: the server's fill brings the rest of
   the folder anyway.
9. That fetch ahead must **not** start a fill of the next folder. Reading a
   page with `GET /api/file` arms a fill of the page's folder, and a fill of a
   second folder replaces the one running, so fetching the next volume's first
   pages that way would stop the fill of the volume being read. Add a way to
   read one file that places it without arming a fill (for example a query
   parameter on the file route), decide where that read's findings go since
   the fill is not there to carry them (the fetch's KL-15 / PK-21 findings),
   state the rule in `docs/spec/`, and use it for the fetch ahead. Once the
   focus actually moves into the next folder, its pages are read as usual and
   the fill follows.
10. The same read lets the sheet show the cover of a folder not on this device
    yet: today the cover is only peeked at and the sheet says
    `Cover not on this device yet` (`sheet.ts`, `COVER_NOT_HERE`), so as not to
    set a whole book coming while the person walks past it. Read the cover with
    the no-fill read instead, so walking past fetches one image and starts no
    fill.

Write code and comments so they do not tie the explorer to one kind of
content ("a book whose pages are image files", "volumes" as the folders of a
series).

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing; add a journey that reads
across the end of one folder into the next.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Unit tests cover ↑ / ↓ with the reader open across folder boundaries at
  different depths, onto a file the reader cannot show, and at both ends of
  the Library; and ↑ / ↓ with the reader closed unchanged.
- [x] A server route test covers next / previous file in Entry Path order,
  including across folders that were never listed.
- [x] A test covers the fetch ahead starting within 10 files of a folder's end
  and the no-fill read not arming a fill (the running fill is not replaced).
- [x] A test covers the sheet reading a not-yet-fetched cover without arming a
  fill.
- [x] An e2e journey reads past the last page of one folder into the first
  page of the next with ↓.

### Before merge (verified outside the check command)

- [x] On `make local`, with a series of several folders not on this device,
  the agent reads to the end of one folder and on into the next with ↓, and
  records in the PR how long the first page of the next folder took to show
  with the fetch ahead, against a run without it (for example by reading
  straight into an unlisted folder).
