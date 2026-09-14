---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rniE "^\s*(//|///|//!)[^\"]*\bpart (way|company)\b" backend/crates/apps/coffret-server && ! grep -rniE "^\s*(//|///|//!)[^\"]*\bno part (becomes|of the (Library|drop))" backend/crates/apps/coffret-server && grep -q multipart docs/spec/loopback-access/README.md && grep -rqF "no folder on this device holds this part of the Library" backend/crates/apps/coffret-server/src/api_error/ && ! grep -niE "^\s*(//|\*).*after part of (it|the drop)" frontend/packages/apps/web/src/dropped.ts frontend/packages/gateway/api/src/upload.ts'
assignee: null
branch: task/0914-1054-say-part-only-of-one-part-of-a-multipart-request
created_at: 2026-09-14T10:54:47Z
updated_at: 2026-09-14T11:46:03Z
---

# docs(server): say part only of one part of a multipart request

## Overview

The loopback-access register (`docs/spec/loopback-access/README.md`, LA-9 to
LA-11) uses `part` as a noun with one meaning: the share of one upload request
that carries one file — the thing a ceiling of 1 GiB applies to, the thing
counted against 4096, the thing the volume is asked about before it is taken.
The server code that implements those rules uses the same noun in the same
sense throughout `backend/crates/apps/coffret-server/src/routes/upload/`
(`Refusal::Part`, `receive` taking "one part", the per-part refusal list).

Beside it, the same crate's prose uses `part` in its ordinary English sense,
often in the same paragraph: "no part of the Library changes until that
flow commits" and "no part becomes visible before it is complete" sit a few lines from
"a part refused for itself"; the upload module's header says a refusal about
one file and one about the whole request "part company"; `receive.rs` and
`file.rs` say a failure "part way through"; `allowance.rs` says a request's
ceiling holds "across every part of it" — where `part` is, for once, the
technical one, and the reader cannot tell. The explorer's upload path has the
same collision: `dropped.ts` and `upload.ts` say a drop can break "after part
of it has landed" a line away from "the parts nothing was written for".

Apply one rule across the server crate (`src/` and `tests/`) and the
explorer's upload path (`frontend/packages/gateway/api/src/upload.ts`,
`frontend/packages/apps/web/src/dropped.ts`, `dropped.test.ts`, and the drop
comments in `App.tsx`): in comments, doc comments and test prose, `part`
names one part of a multipart request and nothing else. Reword every other
use — a portion of the Library, a share of a drop, "part way", "part
company", "the part with a consequence", "which part of the shape" — with a
word that says what is meant (a region of the Library, some of the drop,
midway, go separate ways, and so on). Read each sentence after the change: a
rewording that leaves the sentence false or ungrammatical is worse than the
ambiguity it removed.

Two things are guarded, not rewritten. The sentence a person reads when no
folder on this device maps a path — "no folder on this device holds this part
of the Library" — is the API's, is asserted by tests on both sides, and is
read by a person and never by the register; keep it verbatim in
`api_error/mod.rs`, `from_error.rs` and the tests that quote it, and where a
comment quotes it, quote it. Likewise leave the diagnostic sentence in
`routes/file.rs` ("an Entry's plaintext stopped part way out") and every
identifier (`part_bytes`, `Refusal::Part`, test names) alone: this task changes
prose, not values the runtime or the tests read.

The register should carry the definition the code relies on. LA-9 speaks of
"any one part" without saying what a part is; add to LA-9 the one clause that
does — a part is one field of the multipart request, whether or not it names a
file — in the register's own voice, so the reader of the crate can follow
`(spec: LA-9)` to the noun's meaning. Nothing else in the register changes,
and no rule's meaning changes; this is a definition the rules already assume.

The Bech32m "human-readable part" in `frontend/packages/domain/format` is a
different term from a different specification and is not in scope.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No comment or doc comment in `backend/crates/apps/coffret-server` says
      "part way" or "part company", or "no part becomes" / "no part of the
      Library" / "no part of the drop".
- [x] No comment in `dropped.ts` or `upload.ts` says a drop broke "after part
      of it" or "after part of the drop".
- [x] LA-9 in `docs/spec/loopback-access/README.md` states what a part is, in
      a clause that names the multipart request.
- [x] The API sentence "no folder on this device holds this part of the
      Library" is still present verbatim under `api_error/`, and `make check`
      passes with the tests that assert it unchanged.
