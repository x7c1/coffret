---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1025-2025-define-rules-in-the-register-and-name-every-test-bound-rule
created_at: 2026-10-25T11:25:21Z
updated_at: 2026-10-25T12:13:59Z
---

# docs(spec): define rules in the register by default, require a test to name every implemented test-bound rule, and mark what is not built yet

## Overview

The spec register's README still describes rules of `Form: test` as interim
text that moves into a test and leaves the register, which shrinks to nothing.
In practice almost every rule is sampled by several tests across crates and
Storage backends, so no single test can hold its definition; only three rules
ever moved. Change the register's model, its checks, and the tree to match.

### 1. The model (`docs/spec/README.md`, `docs/concepts/README.md`)

Rewrite "Rule Forms" and "Rule IDs" in `docs/spec/README.md`:

- **Form says what expresses a rule and nothing else.** `Form: test` is a rule
  a test checks; `Form: prose` is a rule only prose can state (keep the
  existing guidance on choosing between them). Form no longer implies a move.
- **A rule is defined in exactly one place, by default the register.** A
  `Form: test` rule whose tests are confined to one module may have its full
  statement and ID moved into that test's module doc (`//! XX-n: …`), with the
  register entry deleted in the same commit, as SA-8, SA-9 and KD-12 already
  are. Not moving a rule is not unfinished work.
- **Rule–test correspondence.** Every rule is `Form: prose`, or `Form: test`
  and named by at least one test that samples it, or marked `unimplemented`.
  A test names a rule by a comment that opens with the rule's ID followed by a
  colon: `// XX-n: …`, `/// XX-n: …` (conformance cases are doc-commented) or
  `//! XX-n: …`, and a case sampling several rules lists them, `// XX-1, XX-2: …`.
- **`unimplemented`.** A rule whose behavior is not built yet carries
  `*(Form: test; unimplemented)*`; the change that builds it adds the test and
  drops the mark together. A rule only some of whose clauses are not built
  yet carries no mark; it says inside the rule which clause is not built yet,
  as a rule of mixed Form says which part is which, and a test names it for
  the part that is.
- **A rule coffret decided not to build is not kept.** Remove it, or state as
  a rule that coffret does not do it. IDs are still never renumbered or
  reused.
- Drop the completeness paragraph and every sentence about the register
  shrinking or rules vanishing; keep the citation rules (bare inside the
  register, `(spec: XX-n)` elsewhere, a test comment may open with the ID).

`docs/concepts/README.md` says citations "survive the register's shrinkage";
reword that clause to what still holds (a citation resolves by searching the
repository, wherever the rule is defined).

### 2. The checks (`scripts/spec-rule-ids.sh`, `scripts/spec-rule-ids-test.sh`)

Extend `spec-rule-ids.sh` so that it also refuses:

- a `Form: test` rule in the register (a rule whose Form tag includes `test`)
  that carries no `unimplemented` mark and that no test names, in the naming
  form above, under `backend/` or `frontend/`;
- a rule marked `unimplemented` that a test names — the behavior exists, so
  the mark was left behind.

A rule that has moved into a test's module doc is named by that module doc.
Read the Form tag as the register writes it (`*(Form: test)*`,
`*(Form: prose)*`, the mixed forms the register already uses, and the new
`*(Form: test; unimplemented)*`); a tag naming `Form: prose` only is not
test-bound. Keep the header comment's account of what the check can and
cannot see accurate. Add cases to `spec-rule-ids-test.sh` for both new
refusals and for a tree that passes (a test-bound rule named by `///`, one
named in a list, one marked `unimplemented` and named by nothing).

### 3. The tree

The following was established by reading each rule against the code. Verify
each claim as you apply it; if a claim does not hold, apply what the code
actually shows and say so in your report.

**Name the rule at an existing test that samples it** (add or correct the
opening `XX-n:`; where a test already opens with the ID in another shape, such
as `// LA-8.` or `// LA-3, the half…`, bring it to the naming form). Paths are
under `backend/crates/`; `uc` is `domain/coffret-usecase/src`:

| Rule | Test |
|---|---|
| CP-7 | `uc/commit_conformance/rebase_tests/renames.rs` `a_rename_of_an_entry_renamed_meanwhile_conflicts` |
| CP-8 | `uc/commit_conformance/happy_path.rs` `a_removal_leaves_the_current_set_and_is_trashed` |
| CP-10 | `uc/commit_conformance/happy_path.rs` `a_commit_makes_the_batch_the_current_state` |
| CP-11 | `uc/index_conformance/replay.rs` `a_replay_reaches_what_a_restore_of_the_head_would` |
| CP-12 | `coffret-format/src/purpose.rs` `each_control_object_kind_maps_to_its_own_purpose` |
| CP-13 | `coffret-format/src/control/journal_record/round_trip_tests.rs` `the_generation_and_the_epoch_come_from_the_framing` |
| CP-17 | `uc/sync_conformance/integrity.rs` `a_provider_hash_mismatch_is_refused` |
| EP-15 | `uc/sync_conformance/departed.rs` |
| KL-5 | `uc/commit_conformance/repair/refusals.rs` `a_keyring_no_replica_answers_stays_unreadable` |
| LA-2 | `apps/coffret-server/tests/routes/fences.rs` `no_route_answers_a_request_that_shows_no_key` |
| LA-3 | `apps/coffret-device/src/server_key.rs` `the_file_holds_the_key_the_server_will_accept` |
| LA-4 | `apps/coffret-device/src/server_key.rs` `a_second_run_draws_a_key_of_its_own` |
| LA-5 | `apps/coffret-server/src/authorize/tests.rs` `a_request_that_arrived_by_somebody_elses_name_is_refused` |
| LA-8 | `apps/coffret-device/src/server_lock.rs` `a_second_server_for_one_library_is_refused` |
| LA-29 | `apps/coffret-device/src/drops/tests.rs` `a_symbolic_link_is_neither_followed_nor_taken_and_is_reported` |
| OC-3 | `uc/sync_conformance/attempted_slots.rs` `an_attempt_whose_slot_another_writer_took_is_disposed` |
| OC-6 | `uc/commit_conformance/refusals.rs` `an_untrashed_removal_reports_what_storage_refused` |
| OC-7 | `uc/sync_conformance/caught_up.rs` `a_catch_up_completes_a_commit_whose_refresh_failed` |
| OC-9 | `uc/freeze_conformance/reservations.rs` `a_freeze_settles_an_interrupted_spool_before_it_builds` |
| PK-5 | `uc/freeze/segment.rs` `an_over_target_entry_forms_a_singleton` |
| PK-8 | `uc/freeze_conformance/import.rs` `a_prefix_narrows_the_run_to_one_folder` |
| PK-9 | `uc/delete_conformance/rebuild.rs` `a_folder_spanning_several_packs_is_deleted_in_one_batch` |
| PK-18 | `uc/freeze_conformance/source_change.rs` `a_file_that_shrinks_under_the_run_stops_its_pack` |
| PK-22 | `uc/fetch_conformance/partial.rs` `a_mangled_chunk_in_a_partial_fetch_is_refused` |
| RV-2 | `uc/fetch_conformance/keyring.rs` `a_key_lost_container_is_reported_and_the_rest_is_fetched` |
| RV-3 | `uc/fetch_conformance/round_trip.rs` `a_second_device_fetches_a_synced_folder` |
| SA-2 | `gateway/google-drive-store/src/oauth/authorization/loopback_redirect.rs` tests |
| SA-4 | `gateway/google-drive-store/src/oauth/authorization/tests.rs` `refuses_a_grant_wider_than_drive_file` |
| SA-5 | `gateway/google-drive-store/src/oauth/granted_scopes.rs` `what_was_granted_is_named_in_full` |
| SA-6 | `gateway/google-drive-store/src/refresh_tests.rs` `a_rejected_token_is_refreshed_once_and_the_call_retried` |
| SA-10 | `apps/coffret-server/tests/routes/reconnect.rs` `a_grant_that_ran_out_is_renewed_and_the_catalog_caught_up_without_a_restart` |

**Name the rule for the clauses that are built, and say inside the rule which
clause is not built yet** (no mark):

| Rule | Test for what is built | Not built yet |
|---|---|---|
| CK-8 | `uc/commit_conformance/checkpoint.rs` `a_checkpoint_is_written_once_the_threshold_is_crossed` | the triggers before a prune and at an epoch activation |
| KL-4 | `uc/commit_conformance/checkpoint.rs` `a_checkpoint_is_written_once_the_threshold_is_crossed` | preserving the evidence after covered records are pruned |
| KL-10 | `uc/commit_conformance/happy_path.rs` `a_removal_leaves_the_current_set_and_is_trashed` | generations across a Master Key rotation |
| KL-11 | `uc/fetch_conformance/keyring.rs` `a_mangled_first_keyring_replica_falls_back` | the repair before a prune or a rotation |
| KL-12 | `uc/sync_conformance/retained_keyring.rs` `keyring_replicas_no_head_names_are_reported_and_kept` | disposing of a proven orphan set (OC-2, OC-3) |
| RV-7 | `uc/commit_conformance/repair/refusals.rs` `a_keyring_no_replica_answers_stays_unreadable` | listing and reporting the affected Containers and Entry Paths |
| OC-6 | `uc/commit_conformance/refusals.rs` `an_untrashed_removal_reports_what_storage_refused` | a later run trashing an untrashed removal |

(OC-6 appears in both tables; apply the second.)

**Mark `unimplemented`**: CK-4, CK-5, CK-6 (prune is not built), CP-6, MR-1,
MR-3 (the test-bound part), MR-4 (Master Key rotation is not built), RV-4
(salvage is not built). Where a sub-bullet of RV-4 is already sampled
(`uc/delete_conformance/rebuild.rs`
`a_pack_whose_kept_entry_was_renamed_is_rebuilt_under_its_current_path`
opens with `/// RV-4, FM-9:`), the rule is partly built: treat it as the
second table instead, naming the built part and saying which part is not.

**Remove OC-5.** coffret decided not to build it (a MAY rule). Delete the
entry, keep the number unused, and drop it from KL-12's list of orphan-cleanup
rules and from any other citation.

OC-1 is already `Form: prose`; leave it.

Where a rule's statement is only partly test-bound in a way the table does not
capture (for instance KL-4's sentence that a candidate's reference in a
Snapshot does not commit it, which may be prose), decide, keep the change
small, and say what you decided and why in your report.

### Out of the change

Do not change any rule's normative content: the edits to rule text are the
Form tags, the `unimplemented` mark, the sentence saying which clause is not
built yet, and the removal of OC-5. Do not change code behavior.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/spec/README.md` states the model above, and no sentence in it or
      in `docs/concepts/README.md` still says the register shrinks or that
      `Form: test` rules move out of it.
- [x] `spec-rule-ids.sh` refuses a test-bound rule no test names and a named
      rule marked `unimplemented`, and `spec-rule-ids-test.sh` shows both
      refusals and a passing tree.
- [x] The repository passes the extended check: every implemented
      test-bound rule is named by a test, and OC-5 is gone.
- [x] `make check` passes.
