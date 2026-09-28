---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && [ "$(git grep -cE "\([A-Z]{2}-[0-9]+[,)]" -- backend frontend ":!*.md" | wc -l)" -eq 0 ] && grep -qE "^check:.*spec-citations" Makefile && ! git grep -qE "ranged reads?" -- backend frontend docs/concepts docs/spec'
assignee: null
branch: task/0928-1429-cite-spec-rules-with-the-spec-prefix-outside-the-register
created_at: 2026-09-28T14:29:30Z
updated_at: 2026-09-28T14:47:45Z
---

# docs: cite spec rules as `(spec: XX-n)` outside the register, and keep it that way

## Overview

`docs/spec/README.md` (around lines 66-81) sets how a rule is cited: bare, as `(KD-4)`, only inside the register; everywhere else — a doc comment, a test comment — with the `spec:` prefix, as `(spec: KD-4)` or `(spec: EP-9, EP-10)`, so a reader who is not standing in the register sees at a glance that the token resolves there. A test comment naming the rule its case samples may open with the ID itself (`// KD-4: …`).

The code carries 572 lines of bare citations across 156 files (`git grep -E "\([A-Z]{2}-[0-9]+[,)]" -- backend frontend ':!*.md'`; `.rs` 324 lines, `.ts` 248). The most are in `backend/crates/domain/coffret-format/src/error/mod.rs` (30), `backend/crates/apps/coffret-interop/src/generate/control_payloads.rs` (21) and `frontend/packages/domain/format/src/control/keyring.ts` (18).

1. **Rewrite every bare citation with the prefix.** `(XX-n` becomes `(spec: XX-n`; a list keeps one prefix at its head, so `(FM-2, FM-9)` becomes `(spec: FM-2, FM-9)`. Leave alone: the register itself (`docs/spec/`), the `// KD-4: …` opening form, and task files under `docs/tasks/`, which are records of past work. Six of the lines are strings, not comments — an assertion message in `backend/crates/gateway/google-drive-store/tests/pre_minted_id_reuse.rs` (around line 69) and five `describe` titles in `frontend/packages/domain/format/src/control/{indexSnapshot,journalRecord,keyring}.test.ts`. They are citations read by a person too; rewrite them the same way. Every prefix the pattern matches today is one the register defines (FM, KD, CP, CK, KL, EP, MR, PK, SA, RV, EL), so it finds no false positive.
2. **Keep it from coming back.** Add a `spec-citations` target to the `Makefile` and make it a prerequisite of `check`, beside `deps` and `interop` (`check: deps interop spec-citations`). It fails, and prints the offending lines, when the pattern above finds a bare citation under `backend/` or `frontend/` outside Markdown, with the same exclusions. Give it a `## spec-citations:` help line and a comment in the style of the neighbouring grep checks (for example the one around lines 527-536 that refuses spike routes), naming `docs/spec/README.md` as the rule it holds. Show it refuses: put one bare citation back, see the target fail and name the line, then take it out again.
3. **Eleven comments say *ranged read* where the register and the code say *range read*.** The concepts and the spec say *range read* (`docs/concepts/container/README.md` calls it "the register's word for what a range read asks for"; `docs/spec/pack-construction/README.md`), and the module is `fetch/range_read.rs`, but eleven doc comments, test comments and one `expect` message in `coffret-usecase` say *ranged read* (`git grep -nE "ranged reads?" -- backend frontend`: `answer_length.rs`, `byte_stream.rs`, `conformance/transfer.rs`, `error.rs`, `fetch/entry_request.rs`, `fetch/range_read.rs`, `fetch_conformance/partial.rs`, `fetch_conformance/shortening_store.rs`). Say *range read* in each, adjusting the article or the sentence where the change needs it.

The diff is large and mechanical: it touches only comments, those six strings and the one `expect` message, so `cargo fmt --check`, the build and every test are unchanged by it.

Review widened the check beyond the pattern above: it catches an ID that opens the parentheses whatever follows it (`(KD-4 and KD-5)`, `(EP-9–11)`), and it fails when `git grep` cannot search at all instead of passing an empty answer as a clean tree. What it cannot see — an ID behind another element inside the parentheses, as in `([Error::X], EP-1)`, or one on the line after its `(` — is named in its comment; the one such citation in the tree, in `coffret-format/src/meta/stored_path.rs`, is rewritten by hand.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No bare `(XX-n` citation remains under `backend/` or `frontend/` outside Markdown (gate on the `git grep` count)
- [x] `make check` runs a `spec-citations` check that refuses a bare citation (grep gate on `check`'s prerequisites, and `make check` passes with it)
- [x] No comment under `backend/` or `frontend/`, and no concept or spec document, says *ranged read* (gate on `git grep`)
- [x] `make check` passes
