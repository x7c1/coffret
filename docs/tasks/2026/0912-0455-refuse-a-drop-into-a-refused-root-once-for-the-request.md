---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment, error-type-design, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "Error::RootRefused" backend/crates/apps/coffret-server/src/routes/upload/refusal.rs && grep -q "fn a_drop_into_a_refused_root_is_refused_whole_rather_than_part_by_part" backend/crates/apps/coffret-server/tests/routes.rs && grep -q "refused_root" frontend/packages/gateway/api/src/upload.ts'
assignee: null
branch: task/0912-0455-refuse-a-drop-into-a-refused-root-once-for-the-request
created_at: 2026-09-12T04:54:18Z
updated_at: 2026-09-12T12:24:02Z
---

# fix(backend): refuse a drop into a refused root once, for the request

## Overview

**This is a behaviour change a person notices**, and it changes the shape of
one answer on the wire. A drop of two hundred files into a mapped folder whose
marker is not the one its mapping recorded is answered today with `200` and
two hundred entries in `refused`, after every byte of the upload has been read
off the wire and thrown away. After this change it is answered with one
`409 declined` / `refused_root`, as soon as the first part reaches the root.

Nothing about *whether* such a drop is refused changes: not one byte was ever
going to land, before or after.

### Why it is one refusal and not two hundred

`Refusal::Request`'s own doc states the condition
(`backend/crates/apps/coffret-server/src/routes/upload/refusal.rs:8-11`):
"About the request, which stops here. What has outrun a budget or the room on
this device is no truer of the next part than of this one, so there is nothing
to be gained by reading the rest."

A refused root satisfies it exactly. The root's marker is read once per part
from the same mapping and cannot change between the parts of one request; the
refusal is not less true of the next part than of this one. Today it is
nevertheless a `Refusal::Part`, because `coffret_device::Error` reaches
`Refusal` through a blanket conversion that makes every one of them about one
file (`refusal.rs:26-30`), and `Error::RootRefused` falls through it.

The rest of the server already reads it the other way. A fill asks the same
question of the same state and answers it "not about one Entry"
(`coffret-server/src/fill/run.rs:131-161`), and the comment at lines 156-158
is this change's argument in the fill's own words: "A whole mapping, and so
the least Entry-specific answer there is: every Entry under that root meets
the same refusal, and asking for the next file would be asking the broken
question again (spec: EP-13)." `fill/run.rs`'s doc at lines 115-124 says why
that question is asked of the value and not of the wire kind. The upload route
is the one flow still deciding it per part.

EP-13 puts the same reading in the spec (`docs/spec/entry-path/README.md:193`,
the bullet beginning "The device **refuses to place** when"): the refusal
"propagates the way a declined placement does (EP-11): a folder fetch
continues past it, while a single writer fails that request as a whole." A
drop is a single writer, and "that request" is the drop.

### What it costs, and why the route already pays it

The route's module doc is honest about the trade and must be rewritten rather
than worked around. It currently says, at
`backend/crates/apps/coffret-server/src/routes/upload/mod.rs:123-125`: "A part
that is refused still has its bytes read off the wire and dropped. The
alternative is answering in the middle of a request the browser is still
sending, which no browser reads." And at lines 136-145 it says what makes
answering mid-request worth it for a budget: "Reading a refused part to the
end costs what that one part costs and keeps the rest of the drop going;
reading out a request that has already passed a budget is doing the whole of
the thing the budget is there to refuse."

A refused root lands on the budget side of that line, not the part side.
There is no rest of the drop to keep going — every remaining part meets the
same root — so reading the request out is doing the whole of the thing the
refusal is there to prevent, and the one part's cost the first paragraph
weighs is in fact the whole request's. The price is the one the budgets
already pay: what reaches the person may be a transfer that failed rather than
the sentence, which is why the sentence is also put in the log.

## What to change

1. **`backend/crates/apps/coffret-server/src/routes/upload/refusal.rs:26-30`** —
   `From<coffret_device::Error> for Refusal` must stop sending a refused root
   to `Part`. Match the one state out and leave everything else where it is:

   ```rust
   impl From<coffret_device::Error> for Refusal {
       fn from(cause: coffret_device::Error) -> Self {
           match cause {
               // The root every part of this drop goes through, and it is the
               // same root for each of them: the refusal is no less true of the
               // next part than of this one, which is this type's own condition
               // for stopping the request (spec: EP-13).
               Error::RootRefused { .. } => Self::Request(cause.into()),
               other => Self::Part(other.into()),
           }
       }
   }
   ```

   Keep the comment above the impl (lines 14-19) true: it says a refusal that
   stops the whole request "is spelled out at each of the few places that mean
   it, and nothing becomes one by falling through a conversion." It is now
   spelled out here too, which is the point — a refused root does not fall
   through, it is matched. Say that rather than deleting the paragraph.
   `Error::Fetch { cause: FetchError::RefusedRoot { .. } }` does not arrive on
   this route — `Error::descent` maps a refused descent straight to
   `Error::RootRefused` (`coffret-device/src/error.rs:980`) — so do not invent
   an arm for it; if the author decides to match it anyway, the comment must
   say it is defensive.

2. **`backend/crates/apps/coffret-server/src/routes/upload/refusal.rs:4-12`** —
   extend `Request`'s doc. It names budgets and room today; it now also covers
   a state of this device's configuration that no part of the drop escapes.
   Keep the stated condition as the thing the variant is chosen by, since that
   is what this change is decided from.

3. **`backend/crates/apps/coffret-server/src/routes/upload/mod.rs:100-145`** —
   the module doc's two sections split the refusals the wrong way now. Move the
   refused root out of "Refused before anything lands" (its sentence is at
   lines 109-111) and into the section about what stops the request. Rewrite
   the aggregate counts in the same pass — "Six refusals" at line 102 and
   "Those six are about the drop itself. Three more are about this server and
   this device" at lines 129-130 — so the prose states what each group *is*
   rather than how many are in it; a count in a doc comment is one more thing
   to keep in step.

   Rewrite lines 123-125 so the claim is scoped to the refusals it is still
   true of, and add the reason a refused root is not among them: there is no
   rest of the drop for reading the part out to protect. Then extend the
   paragraph at lines 136-145 to cover it, since it is the paragraph that
   already accepts answering in the middle of a request and says what that
   costs.

   The loop itself (lines 231-257) does not change: the arm at line 256
   already returns the refusal, and the comment above it at lines 249-255 —
   nothing is armed for what landed before it — is true of this refusal too. A
   drop into a refused root lands nothing, so there is nothing for it to be
   true *of*; leave the comment as it is rather than qualifying it.

4. **`backend/crates/apps/coffret-server/src/routes/upload/receive.rs:22-46`** —
   the doc lists what each of the two kinds of refusal covers, and lines 28-29
   put the refused root among the per-file ones ("its mapped root is not the
   root the mapping was recorded against, refused as that root is opened").
   Move it to the paragraph at lines 40-42, which is where the request-level
   refusals are described, and let that paragraph say what the two now have in
   common: neither is truer of the next part than of this one. No code in this
   file changes — the refusal still arrives through `?` on
   `library.receive_file(&path)` at line 60, and the conversion decides how far
   it reaches.

5. **`backend/crates/apps/coffret-device/src/add/receive_file.rs:58-63`** — the
   `# Errors` paragraph explains the whole-request reading as "this device is
   placing the one file it was handed and has no other mapping to go on with".
   That is true and now understated: on the upload route it is also every other
   part of the same drop meeting the same root. Add the clause; the reasoning
   in this crate should not be narrower than the route's.

6. **`backend/crates/apps/coffret-server/tests/routes.rs`** — add
   `a_drop_into_a_refused_root_is_refused_whole_rather_than_part_by_part`,
   beside `a_drop_onto_a_folder_that_is_not_on_this_device_is_refused_whole`
   (line 944), which is the shape to follow. Rewrite the mapped root's marker
   the way the fetch-side test does at line 534, then
   `served.upload(<folder>, &[(..), (..)])` with two parts and assert:

   - the status is `409` and the body is `declined` / `refused_root`, with the
     message the same one a fetch is refused with;
   - the answer carries no `refused` array — the refusal is the answer, not an
     entry in one;
   - neither part landed (`!served.holds(..)` for both), and no sync or freeze
     was armed, which the test at lines 952-957 asserts by reading
     `/api/activity`.

   One assertion is the point of the whole change and should be commented as
   such: **two parts, one refusal.** The existing
   `a_part_the_library_holds_inside_a_pack_is_refused_and_its_sibling_lands`
   (line 964) is the case that must keep working unchanged — a per-file refusal
   still lets the file beside it land — so do not weaken it.

7. **`frontend/packages/gateway/api/src/upload.ts:30-42` and `62-77`** — no
   code changes, and two docs that are now wrong. `addFiles`'s doc at lines
   74-76 says "A refusal thrown out of this is about the drop as a whole: the
   folder is not on this device, so there is nowhere to put any of it" — as
   though `unmapped` were the only one. Name `refused_root` as the second, and
   say what distinguishes them for a reader: one is a folder this device has
   no folder for, the other a folder it has and will not write into.
   `Upload`'s doc at lines 33-36 ("Per part, because a drop is a handful of
   files and they are separate questions") should say which questions are not
   separate.

   The screen needs nothing: `App.tsx:366` already puts a thrown refusal's
   sentence on the screen through `said` (`web/src/useRemote.ts:73-79`), which
   returns `Refusal.message` verbatim, and `refused_root` is already in
   `DeclinedReason` (`refusal.ts:69-75`) and in the `REASONS` list
   (`refusal.ts:219`). Do not add a branch for it — there is nothing a page
   can do about a mapping, which that doc comment already says.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] the conversion decides the reach of a refused root rather than letting it
  fall through:
  `grep -q "Error::RootRefused" backend/crates/apps/coffret-server/src/routes/upload/refusal.rs`
- [x] a route test holds the drop to one refusal:
  `grep -q "fn a_drop_into_a_refused_root_is_refused_whole_rather_than_part_by_part" backend/crates/apps/coffret-server/tests/routes.rs`
- [x] the client's account of what is thrown out of a drop names the second whole-drop
  refusal:
  `grep -q "refused_root" frontend/packages/gateway/api/src/upload.ts`

## Out of scope

- **What the refusal says.** The sentence, and the mapping it names, are
  settled by the change this one is stacked on; nothing here edits
  `api_error/mod.rs` or `noted.rs`.
- **The other per-part refusals.** A part refused by name — not an Entry Path,
  a reserved component, an Entry the Library holds inside a Pack — and a part
  whose descent is blocked below a sound root stay `Refusal::Part`: each is
  about that one name and says nothing about the part beside it. Only the root
  every part shares moves.
- **Reading out the body a refused request leaves in flight.** Answering in
  the middle of a request the browser is still sending is what the budgets
  already do, with the cost the module doc states; this change joins that
  behaviour rather than changing it. Draining the remainder before answering,
  or answering after a bounded drain, is a change to every request-level
  refusal on the route and belongs on its own.
- **Anything under `frontend/packages/apps/`.** The screen's handling of a
  thrown refusal is already correct for this state.
