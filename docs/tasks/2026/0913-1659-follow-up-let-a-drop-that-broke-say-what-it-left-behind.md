---
status: completed
pipeline_phase: null
base_ref: null
follow_up_of: docs/tasks/2026/0913-1611-let-a-drop-that-broke-say-what-it-left-behind.md
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "not proof the server never answered" frontend/packages/gateway/api/src/refusal.ts && grep -q "A drop every part of which was refused has" backend/crates/apps/coffret-server/src/routes/upload/mod.rs && grep -q "Follow work a drop may have just armed" frontend/packages/apps/web/src/useActivity.ts'
assignee: null
branch: task/0913-1659-follow-up-let-a-drop-that-broke-say-what-it-left-behind
created_at: 2026-09-13T16:59:45Z
updated_at: 2026-09-13T17:31:29Z
---

# docs: say what a broken upload leaves unknown, in the four places that overstate it

## Overview

Four comments state, each in its own way, something stronger than what the
code now guarantees. A drop of many files is one request that can be refused
after some of them have landed, and whose answer may never be read; each of
these three was written when the narrower reading was still true. **No
behaviour changes here — comment and doc-comment text only.**

### 1. `frontend/packages/gateway/api/src/refusal.ts`

Line 46, the doc comment immediately above the `| 'unreachable'` member:

```
  /** The request never got an answer: nothing is listening, or the network went. */
```

It names two causes as though they were the whole set, and omits the third — a
transfer that broke while the body was still going up. `addFiles`
(`frontend/packages/gateway/api/src/upload.ts`) says plainly that "`unreachable`
out of this function is not proof the server is gone", so the type definition,
which is where a reader learns what the kind means, currently contradicts the
function that throws it.

Replace that single line with:

```
  /**
   * The request got no answer here: nothing is listening, the network went, or
   * the transfer broke while the body was still going up — which is
   * not proof the server never answered. Every `fetch` that rejects becomes
   * this, whatever it rejected for, except one the caller aborted: that is not
   * a refusal at all and passes through as itself.
   */
```

### 2. `backend/crates/apps/coffret-server/src/routes/upload/mod.rs`

Lines 277-279, anchored by `// Only where something landed. A drop that was refused whole has left the`:

"refused whole" reads as the request-level refusal, but this file's own module
documentation says such a drop leaves "what landed before it ... in the folder,
with nothing armed", and `a_drop_of_more_parts_than_one_gesture_carries_is_stopped`
asserts exactly that. The comment is true only of an answer every part of which
was refused.

Replace those three comment lines with:

```
    // Only where something landed. A drop every part of which was refused has
    // left the folder exactly as it was, and a run over an unchanged folder is
    // a walk to find nothing. A drop stopped for the whole request is answered
    // where it was stopped and never reaches here.
```

### 3. `frontend/packages/apps/web/src/useActivity.ts`

Line 43, anchored by `   * Follow work a drop has just armed, before any answer has said so.`:

`follow()` is now also called where the request never answered, so nothing is
known to have been armed. Replace that line with:

```
   * Follow work a drop may have just armed, before any answer has said so.
```

The comment above `setFollowing` in the same file states the stronger claim in
plain prose — "A drop arms its flow before it answers, so the server is already
running one by the time this page hears the upload landed" — which is true only
of the path where an answer arrived. It gains a sentence for the other path:

```
  // for the activity since. A drop that broke mid-transfer turns this on too: it
  // may have broken after that same arming, and nothing else would start the
  // asking. Without this the first tick would be the one after something else
  // happened to start the polling, which for a drop onto a folder with no reader
  // open is never.
```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `refusal.ts`'s `unreachable` doc names the broken-transfer case and no
      longer reads as proof the server never answered —
      `grep -q "not proof the server never answered" frontend/packages/gateway/api/src/refusal.ts`.
- [x] `mod.rs`'s arming comment distinguishes a drop every part of which was
      refused from a drop a budget stopped —
      `grep -q "A drop every part of which was refused has" backend/crates/apps/coffret-server/src/routes/upload/mod.rs`.
- [x] `useActivity.ts`'s `follow` doc no longer asserts that something was armed —
      `grep -q "Follow work a drop may have just armed" frontend/packages/apps/web/src/useActivity.ts`.
