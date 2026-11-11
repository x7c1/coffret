---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1111-1650-bring-stale-docs-and-a-misnamed-constant-in-line
created_at: 2026-11-11T07:50:00Z
updated_at: 2026-11-11T08:05:00Z
---

# docs: bring stale rule text, comments and a misnamed constant in line with the code

## Overview

Each of these says something the code no longer does, or names a thing by a
role it no longer has. Fix each to describe what the code does today; change
no behaviour.

1. **LA-32's request line lacks `under`.** In
   `docs/spec/loopback-access/README.md`, LA-32's opening form reads
   `GET /api/beside?path=<row>[&folder=true]`; the route also takes
   `[&under=<folder>]`, which its body and the route's doc comment already
   describe. Add it to the form.
2. **LA-43 leaves out part of what the explorer does with kept filter
   settings.** The explorer (`frontend/packages/apps/web/src/useKeptFilter.ts`)
   reads them again after an unlock, sends a refused save again, lets what
   was changed before the answer arrived win over the answer, and does not
   redraw when it reads back the values already on screen. Write those
   obligations into LA-43 as the code implements them, and cite LA-43 from
   the tests that sample them. LA-43 also says only that a change it could
   not keep is said in the page's console; the explorer does the same when
   it cannot read the kept settings (`console.warn` in `useKeptFilter.ts`).
   Write that too, as: "A change it could not keep, or kept settings it
   could not read, is said in the page's console and leaves the filter on
   screen as it is."
3. **`authentication_required`'s doc comment** in
   `frontend/packages/gateway/api/src/refusal.ts` (around lines 122–127)
   does not say that, in the desktop app, the app reloads the window after
   the page lets go of its token. Add that.
4. **The route count in `frontend/packages/gateway/api/src/index.ts`** says
   twenty-six; the server's `routes/mod.rs` says twenty-eight. Make the
   gateway's comment agree with the routes the server mounts (count them;
   do not trust either number).
5. **`frontend/packages/apps/e2e/journeys/11-stop.spec.ts` line 13** calls
   itself the last of the journeys; journeys 12–19 follow it. Correct it.
6. **The `make local` seed uses a real author and work.** Under
   `日本の作家/` in `backend/crates/apps/coffret-fixtures/src/seed.rs`, one
   author folder and one work in it (and another work by the same author)
   carry the names of a real novelist and real novels, while every other
   seeded name is made up. Replace them with a made-up author and titles of
   the same shape (Japanese, nested the same way), and update any test or
   document that names them. Do not write the real names anywhere new.
7. **`MODIFIED_END_PAD`** in `frontend/packages/apps/web/src/TreeTable.tsx`
   is the padding after the last date column, which is Added while the
   Added column shows. Rename it to a name that says that (e.g.
   `LAST_DATE_END_PAD`) at every use.

Do not change `.github/`. Do not change what any rule requires beyond
writing down what the code already does.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] LA-32's form names `under`, and LA-43 states the four explorer-side
      obligations; `make spec-citations` and `make spec-rule-ids` (in
      `make check`) pass with the tests that sample them naming LA-43.
- [x] Every author and title the seed places under `日本の作家/` is made
      up: a seed test pins the made-up names the seed now uses there, so
      the real ones cannot return unnoticed.
- [x] No file names `MODIFIED_END_PAD`.
- [x] `make check` and `make e2e-it` pass.
