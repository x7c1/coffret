---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0912-0454-name-the-mapping-a-refused-root-refusal-is-about.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "the folder this device maps \"albums\" into is not the folder" frontend/packages/gateway/api/src/refusal.test.ts && grep -q "the folder this device maps \"albums\" into is not the folder" frontend/packages/apps/web/src/fill.test.ts && ! grep -q "was set up" frontend/packages/gateway/api/src/refusal.ts && grep -q "no Entry Path to name the mapping" backend/crates/domain/coffret-usecase/src/descent_error.rs && grep -q "EL-1" backend/crates/domain/coffret-usecase/src/unavailable_root.rs'
assignee: null
branch: task/0912-0700-follow-up-name-the-mapping-a-refused-root-refusal-is-about
created_at: 2026-09-12T07:23:18Z
updated_at: 2026-09-12T07:57:52Z
---

# docs: say "recorded against" about a refused root everywhere it is described

## Overview

A refused root's refusal now names which mapping it is about, and says so with
the verb this repository uses for recording a mapping. Five places that describe
that state were not part of that change and still describe it the old way, or do
not describe it at all. All five are comment, doc-comment or test-fixture text;
nothing about behaviour changes.

The sentence the server now composes, for a mapping standing for `albums`:

```
the folder this device maps "albums" into is not the folder that mapping was
recorded against, so nothing was put into it; record that mapping again with
`coffret map`
```

1. **`frontend/packages/gateway/api/src/refusal.test.ts`** — its `refused_root`
   fixture carries the sentence the server used to compose, which named no
   mapping. It is a mock input and the case asserts only that the message
   contains `coffret map`, so the fixture is stale rather than failing. Replace
   it with the sentence above.

2. **`frontend/packages/apps/web/src/fill.test.ts`** — the same stale sentence,
   in a local constant both of that file's assertions compare against. Replace
   it with the sentence above. The comment two lines up paraphrases the state
   with "set up against" as well; bring it along.

3. **`frontend/packages/gateway/api/src/refusal.ts`** — the `refused_root`
   variant's doc paraphrases the server's sentence and says the folder "is not
   the folder its mapping was set up against". The repository's verb for the act
   is *record*: the spec rule that governs this state opens "Recording a mapping
   also records an identity for the root folder itself", `coffret map`'s own help
   says a mapping is recorded, and this file's sibling case already writes
   "recorded against" in its own prose. Change "was set up" to "was recorded" in
   that doc. This is the last live description of this state using the other verb.

4. **`backend/crates/domain/coffret-usecase/src/descent_error.rs`** — the
   `Refused` variant's doc says what the value carries and not what it leaves
   out. The capability is handed the root and the path's components apart and so
   has no Entry Path to name the mapping by, which is why a caller that puts this
   refusal in front of a person — which the spec asks to name the mapping — takes
   that name from the row it descended through. Add a paragraph at the end of
   that variant's doc saying so, pointing at `LocalPlace::prefix`.

5. **`backend/crates/domain/coffret-usecase/src/unavailable_root.rs`** — its
   struct doc claims "It never travels into a diagnostic event, and neither does
   the prefix — an Entry Path component is no more loggable than a local path",
   without citing the rule. Everywhere else in the repository that claim ends
   with `(spec: EL-1)`, including the companion struct this state's own value was
   modelled on. Rewrap that sentence to carry the citation.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] Both frontend fixtures carry the sentence the server composes:
      `grep -q 'the folder this device maps "albums" into is not the folder' frontend/packages/gateway/api/src/refusal.test.ts`
      and the same over `frontend/packages/apps/web/src/fill.test.ts`.
- [x] No live description of this state uses the other verb:
      `! grep -q "was set up" frontend/packages/gateway/api/src/refusal.ts`.
- [x] The capability's refusal says what it does not carry:
      `grep -q "no Entry Path to name the mapping" backend/crates/domain/coffret-usecase/src/descent_error.rs`.
- [x] The companion struct cites the rule its claim rests on:
      `grep -q "EL-1" backend/crates/domain/coffret-usecase/src/unavailable_root.rs`.
