---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -qi "added one at a time\|add these files one at a time" frontend/packages/apps/web/src/dropTarget.ts && grep -rq "upload.onprogress\|upload.addEventListener" frontend/packages/gateway/api/src'
assignee: null
branch: task/1007-0444-say-what-a-drop-will-do-and-how-far-an-upload-has-gone
created_at: 2026-10-07T04:47:33Z
updated_at: 2026-10-07T05:08:03Z
---

# feat(explorer): say what a drop will do before it lands, and how far an upload has gone

## Overview

Dropping files on the explorer does one of two different things, and the page
only says so in one case. A drop into a folder made with "new folder" that the
Library does not have yet is a **freeze**: the pages are packed together into
one Pack (spec: PK-7). A drop into any other mapped folder is a **sync**: each
file becomes a Container of its own. Today the made folder's banner says the
first; an existing folder says nothing, so a person cannot tell before letting
go which one they are about to start. And once they let go, the status bar says
"adding N files to <folder>" with nothing moving until the server answers — for
a book of a few hundred pages that is tens of megabytes over the loopback
socket, and then the packing line takes over. During the first real use of the
explorer the person saw no progress at all for the upload and could not tell
whether anything was happening.

**1. The drop target says what it will do.** While files are dragged over the
list (`FileList.tsx` already tracks `dragenter` / `dragleave`), the list shows
one line naming the outcome for the folder under the pointer:
- a made folder the Library does not have yet → "drop to pack these pages together as one book";
- a mapped folder the Library has → "drop to add these files one at a time";
- a folder that is not on this device → the existing refusal (it cannot take a drop);
- a made folder while another book is being packed → the existing "packed after that one" wording.
Reuse the sentences the banners already hold where they say the same thing;
add the missing "added one at a time" sentence for an existing folder, in the
banner too, so the two kinds read as a pair. Keep the decision in a DOM-free
module (the one `bookDrop` and `unmapped.ts` sit beside) with unit tests for
each folder state.

**2. The upload reports its progress.** `fetch` cannot report upload
progress; the api package's upload call (`frontend/packages/gateway/api/src`,
the function the drop uses for `POST /api/upload`) moves to `XMLHttpRequest`
for that one request, keeping its contract (same URL, headers, body, the same
refusal parsing as every other call, abort through the same `AbortSignal`), and
takes an optional progress callback `(sent, total)`. The status bar's adding
line shows it: "adding 300 files to book-000 — 23 MB of 58 MB", updated at most
a few times a second. When the upload finishes and the server starts the
freeze or the sync, the existing packing / backing-up line takes over as it
does today. Unit tests: the progress line's wording for a few sizes; the upload
call's progress callback under a fake XHR (Vitest + jsdom provide one, or a
small hand-rolled stub); a refusal still parsed as before.

**3. The finished book stays said.** Check that the "packed N pages into 1
Pack" notice the freeze leaves (`fill.ts` `packed(...)`) stays on screen until
the person dismisses it, as the reconnect and mapping notices do, rather than
disappearing at the next poll; if it does not, make it stay. If it already does,
change nothing here and say so.

Spec: no rule changes (PK-7 for what a freeze is, EP-9 for mapped folders).
Out of scope: progress inside the server's packing beyond what the work answer
already reports; Pack update and delete propagation; thumbnails.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] The drop target's outcome line is decided by a DOM-free function with unit tests for a made folder, an existing mapped folder, an unmapped folder and a made folder behind another book
- [x] The drag-time line over an existing mapped folder says files dropped there are added one at a time (the line is `dropLine` in `dropTarget.ts`; a permanent banner over every mapped folder was judged noise)
- [x] The upload call reports progress through `XMLHttpRequest`'s upload events, keeps the refusal parsing and abort behaviour, and is unit-tested
- [x] The status bar's adding line shows bytes sent of total, with unit tests for its wording
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Under `make dev` for the development Library, dragging files over a made folder and over an existing folder shows the two different outcome lines, and dropping a few hundred pages into a made folder shows the byte progress and then the packing line — an agent runs this in Chromium if the development pair is up with this build, or reports why not
