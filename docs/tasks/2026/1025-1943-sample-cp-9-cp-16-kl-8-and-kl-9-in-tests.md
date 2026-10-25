---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && for id in CP-9 CP-16 KL-8 KL-9; do git grep --untracked -qE "^\s*//[/!]? ([A-Z]{2}-[0-9]+, )*$id(, [A-Z]{2}-[0-9]+)*:" -- backend || { echo "no test names $id"; exit 1; }; done'
assignee: null
branch: task/1025-1943-sample-cp-9-cp-16-kl-8-and-kl-9-in-tests
created_at: 2026-10-25T10:43:00Z
updated_at: 2026-10-25T11:08:30Z
---

# test: sample CP-9, CP-16, KL-8 and KL-9 in tests

## Overview

Four `Form: test` rules in `docs/spec/` describe behavior the code already
has, but no test samples them. Add one test case (or a small group) for each,
and open each case's comment with the rule's ID — `// CP-9: …` in a unit
test, or `/// CP-9: …` on a conformance case — so the test names the rule it
samples, as the other cases in the suites already do.

Where a case belongs in a conformance suite (`src/*_conformance/` in
`coffret-usecase`), put it there so every Storage backend runs it; follow the
neighbouring cases' fixtures and helpers.

1. **CP-9** (`docs/spec/commit-protocol/README.md`): the previously committed
   Keyring stays authoritative until the Journal commit, so the candidate
   leaves out removed Containers without making the pre-commit state
   unreadable. The behavior lives in `coffret-usecase/src/commit/keyring/`.
   Suggested case: a batch with a removal whose head create is refused (the
   commit conformance suite's faulty stores can refuse the head); the
   Container being removed can still be opened through the committed
   Keyring, and the candidate Keyring does not carry it.
2. **CP-16**: immediately before consuming a slot, a writer re-reads the head
   object the slot came from and aborts if it is gone
   (`coffret-usecase/src/commit/journal.rs`, `require_head`, `MissingHead`).
   Suggested case: a store that removes the head after the catch-up and
   before the slot is consumed; the commit stops with the missing-head
   refusal and no successor head is created. Master Key rotation is not
   implemented, so the case removes the head directly rather than through a
   rotation.
3. **KL-8**: a generation's commitment selects the replica count required
   for that generation; a newly prepared generation uses the current replica
   policy, whose initial value is three (`coffret-usecase/src/commit/commit_policy.rs`).
   Suggested cases: the default policy's replica count is three; a
   generation committed with a count of two is judged by its commitment's
   count of two when it is later read or repaired under a policy of three.
4. **KL-9**: replication is effective only within one generation. Suggested
   case: commit two generations, damage every replica of the newer one and
   keep the older one intact; reading the Library treats the Keyring as lost
   and does not fall back to the older generation.

If writing a case shows the code does not do what the rule says, do not
change the code or the rule: stop and report it as `needs_review`, naming the
rule, the sentence, and what the code does instead.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test names each of CP-9, CP-16, KL-8 and KL-9 with a comment opening
      with its ID (gate appended), and samples the behavior the rule states.
- [x] `make check` passes.
