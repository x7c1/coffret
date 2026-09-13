---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [user-experience, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "revokeObjectURL" frontend/packages/apps/web/src --include=\*.test.ts --include=\*.test.tsx && ! grep -q "    held: usize," backend/crates/apps/coffret-server/src/lock/idle.rs && ! grep -q "which is neither the default nor a constant of the" backend/crates/apps/coffret-server/tests/routes.rs && grep -rq "revokeObjectURL" frontend/packages/apps/web/src/ReaderView.tsx'
assignee: null
branch: task/0913-1423-let-locking-take-back-what-the-key-gave
created_at: 2026-09-13T14:23:40Z
updated_at: 2026-09-13T15:57:41Z
---

# fix(frontend): let locking take back what the key gave

## Overview

Locking ends this server's hold on the Master Key. The screen is told so —
`App`'s `lock` calls the server and then re-asks its three questions, and the
listing and the tree come back refused with the server's own sentence about the
Passphrase. `App` deliberately invents no locked state of its own; the server is
the one that knows, and it is asked.

**But the reader is not one of the three questions.**

`ReaderView` holds the page it is showing as an object URL over a decrypted blob
(`ReaderView.tsx:76`, `:122`), and holds the pages it prefetched around that one
the same way (`:145`, `:107`). `App`'s `retry` reloads `library`, `folders` and
`listing` (`App.tsx:102`–`106`) and touches none of it, and `ReaderView` is
handed nothing that would tell it the key is gone — its props are `pages`, `at`,
`onNavigate`, `onClose`, `onFetching` and `onFetched` (`App.tsx:505`–`512`).

So a person who is reading, clicks the lock, and is told the Library is shut
goes on looking at the page until the refused listing arrives and takes the
reader off the screen with it. **That window is a round trip wide, not
permanent** — the reader's unmount already revokes what it holds — but it is a
round trip during which the one gesture whose whole purpose is to put the
Library away has left the part of the screen showing its contents exactly as it
was. On a slow link, or one that has gone away entirely, that refusal can be a
long time coming or never come at all.

What has no window at all is a page still in flight. A request the reader made
before the lock resolves afterwards, and nothing today tells it that the key it
was asked under is gone.

### The change

When a lock succeeds, the pages this device decrypted go with it: the one on
screen and every one held around it. Revoke the object URLs rather than only
dropping the references — an unrevoked URL keeps its blob alive for the lifetime
of the document.

How the reader learns of it is the design question this task settles. `App`
holds the lock and `ReaderView` holds the pages, and today nothing connects
them. Whatever carries the signal must not become a second locked state for the
screen: the rule `App`'s own comment states — the server is the one that knows —
stays. What is being added is a *discard*, not a *status*.

Leave the reader open on the refusal its next request earns, the way every other
refusal on this screen is shown. A person who locks while reading should end up
looking at the same sentence the listing and the tree are showing, not at a page.

### Two local cleanups in the lock code this opens

Named separately so a reviewer can see they were chosen rather than swept in:

- **`Custody`'s `held` and `Presence`'s `held` are two different things under
  one word, in one module.** `Custody.held` is
  `RwLock<Option<Arc<OpenLibrary>>>` (`lock/custody.rs:19`) — *the thing being
  held*. `Presence.held` is a `usize` (`lock/idle.rs:24`) — *how many holds are
  open right now*. A reader moving between the two files has nothing to tell
  them the word changed from a subject to a count.
- **`QUIET`'s documentation changes subject mid-sentence**
  (`coffret-server/tests/routes.rs:2096`–`2100`). It opens on the value — "A
  quarter of an hour, which is neither the default nor a constant of the
  server" — and after the colon the subject becomes the concept: "how long a
  device stays unlocked is a policy parameter". A reader learns what the
  constant is *not* before learning what this test uses it *for*.

## What must not change

- **`App` invents no locked state of its own.** The server is asked; its
  sentence is what the screen shows. This change adds a discard on a successful
  lock, not a client-side notion of "locked".
- **The status bar keeps the Library's name.** That is not something the Master
  Key kept.
- **A lock that did not happen still says so loudly** — `App.tsx:131`–`137`
  explains why that one refusal is reported differently from every other, and
  the sentence it sets stays.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A successful lock revokes the object URL of the page on screen and of
      every prefetched page, asserted by a test that counts revocations rather
      than only checking that state was cleared.
- [x] A page still in flight when the lock happens is revoked on arrival and
      handed to nobody, including when a second lock happens while it is in
      flight, asserted by tests that fail if the staleness check is narrowed to
      the immediately previous attempt.
- [x] A lock that fails revokes nothing and leaves the reader as it was — the
      pages are still protected by a key the server still holds.
- [x] `App` holds no boolean meaning "the Library is locked"; the existing
      `locking` flag, which means a lock is in flight, is untouched.
- [x] `Custody`'s and `Presence`'s `held` no longer share a name, and every call
      site names what it reads.
- [x] `QUIET`'s documentation says which side of the boundary the constant is.

### Manual / on-hardware (verified by a human before merge)

- [ ] Open a book in the browser, read a few pages, click the lock, and confirm
      the page goes away rather than staying readable. The automated tests pin
      the revocations and the state; this confirms what a person actually sees.
