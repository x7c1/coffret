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
branch: task/1106-0956-filter-the-library-by-name-and-recent-books
created_at: 2026-11-06T09:56:48Z
updated_at: 2026-11-06T10:29:23Z
---

# feat(server): filter the Library by name and by the most recent books

## Overview

The explorer is to get a filter: a name typed in the header, and "the n most
recently deposited books". This change adds the server side only: a route that
answers which rows a filtered tree keeps. The explorer does not use it yet.

Add `GET /api/filter` with query parameters `name` (optional) and `recent`
(optional, a positive integer n); at least one of them is given, else the
request is refused as malformed. It is keyed and locked like every other route
(LA-2, and refused while the Library is locked like `/api/library/totals`).

**Matching a name.**
- The name is matched against each `/`-separated segment of each Entry Path
  in the device's Index (`entries`), never across a `/`. For `a/b/c.jpg` the
  segments are `a`, `b` and `c.jpg`.
- It is a substring match, case-insensitive, with both sides normalized by
  Unicode NFKC first, so full-width and half-width forms match each other.
- A folder is not stored; it is derived from Entry Paths (EP-2). "A folder
  matches" means one of its segments matches.
- The scope is the whole Library. Storage is not touched.

**The n most recent books.**
- A book is a folder that directly holds files. A folder holding only
  subfolders (a series) is not a book. The files directly under the Library
  root count together as one book (the root).
- A book's time is the latest `deposited_at` among the files directly in it.
  The n newest books are taken. Renames and moves do not change
  `deposited_at` (FM-15 / FM-16), so reorganizing does not change the order.

**Both together** is AND. The n most recent books are taken from within what
the name matched; for `name=starfall&recent=5`, the newest five books among
those under or named by a match.

**The answer** lists the matched rows in path order. Each row has its path,
whether it is a folder or a file, and for folders the time used by Recent
(`deposited_at` of the newest file directly in it, where it has one). The
rows of a matched folder's contents are not included: the explorer lists them
as it does today, folder by folder, when one is opened. Ancestors of a match
are not listed either; the explorer derives them from the paths.

**The limit.** At most 2,000 matched rows are returned, in path order. The
answer also carries the total number of matches, counted over the whole scan,
and whether it was cut short, so the explorer can say "Showing the first 2,000
of 8,412 matches".

Measured on a Library of about 100,000 files, a full scan of `entries`
with this matching takes about 90 to 110 ms. So this change does the full scan
and does not change the Index schema. The matching and recent-books logic in
the measuring crate (`backend/crates/apps/coffret-measure`) can be the
starting point. Put the logic where the server's use cases live, following the
crate layering in `CLAUDE.md`, rather than having the server depend on the
measuring crate. The measuring crate may then use the shared code.

**Spec.** Write the route's rules into `docs/spec/loopback-access/README.md` as
a new LA rule (or rules), next to LA-39. Write what is stated above, and name
each rule from the tests that check it, as the other LA rules are.

States each covered by a route test:
- name only, matching folders, files, and Japanese full-width / half-width
  names;
- name that matches nothing (an empty answer, total 0);
- recent only, including files directly under the root counting as one book,
  and a series folder that is not counted;
- name and recent together;
- more matches than the limit (cut short, total given);
- neither parameter, an empty name, `recent=0` or a non-number (refused);
- the Library locked (refused).

Out of scope: any change to the explorer.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Route tests under `backend/crates/apps/coffret-server/tests/` cover
      every state listed in the Overview.
- [x] Unit tests cover per-segment NFKC case-insensitive matching (never
      across `/`) and the choice of the n most recent books.
- [x] `docs/spec/loopback-access/README.md` states the route's rules, and
      `make check`'s spec gates (rule IDs and citations) pass.
- [x] `make check` passes.
