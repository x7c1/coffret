---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && bash -n scripts/e2e-it.sh && test "$(grep -c -- --fail-with-body scripts/e2e-it.sh)" -ge 4 && ! grep -rqw CAPABILITY_HEADER backend/crates && ! grep -rqw KEY_HEADER frontend/packages && grep -rqw SERVER_KEY_HEADER backend/crates && grep -rqw SERVER_KEY_HEADER frontend/packages && ! grep -q "device’s" frontend/packages/gateway/api/src/refusal.test.ts && ! grep -q "that server admits" frontend/packages/apps/web/vite.config.ts'
assignee: null
branch: task/0909-2011-name-the-server-key-header-alike-on-both-sides
created_at: 2026-09-09T20:11:57Z
updated_at: 2026-09-10T04:41:01Z
---

# refactor(server): name the server-key header alike on both sides and keep a refusal's body in the E2E transcript

## Overview

One name for one header. `CAPABILITY_HEADER`
(`backend/crates/apps/coffret-server/src/authorize/mod.rs`) is the only place
in the tree that calls this value a capability; the register
(`docs/spec/loopback-access/README.md`), the concepts and the `ServerKey` type
all say server key. Rename it `SERVER_KEY_HEADER` on both sides — every Rust
reference (the definition, `authorize/shows_the_key.rs`, `authorize/tests.rs`,
the public re-export in `coffret-server/src/lib.rs`, the line `main.rs` prints
as it starts, `tests/routes.rs`, `tests/support/mod.rs`) and the independent
TypeScript constants `KEY_HEADER` in `frontend/packages/apps/web/vite.config.ts`
and `frontend/packages/apps/e2e/journeys/server.ts`. The literal
`x-coffret-key` is unchanged on both sides, so nothing on the wire moves;
`cargo clippy -D warnings` and `pnpm -r typecheck` catch any reference left
behind.

`scripts/e2e-it.sh` asks the routes through `curl --fail`, which discards the
body of a 403 — so when the API stage fails against a refusal, the transcript
carries the assertion and not the server's explanation of it. Move the four
sites that ask the routes to `--fail-with-body` (available since curl 7.76;
CI's `ubuntu-latest` and a developer checkout both run curl 8.x): the `api`
helper (no call site changes — `jq --exit-status` still evaluates false and
the non-zero exit still trips `|| fail`), the `/api/list` failure message
(include the captured body), the `/api/file` site (keep `--output`; the body
lands in the output file, so the failure branch reads it back with a comment
saying why), and the two `/api/upload` sites (drop `--output /dev/null` and
capture instead). Update the helper's comment and the walk's "It is `--fail`ing
curl the whole way" comment. The two MinIO health polls (`scripts/e2e-it.sh`
near the MinIO readiness loop, and `scripts/s3-store-it.sh`) keep `--fail`: a
failure there is the expected answer and the body is dropped on purpose.

Two text defects in files this change already touches. The `unauthorized`
fixture in `frontend/packages/gateway/api/src/refusal.test.ts` quotes the
server's own sentence but spells `device's` with a curly apostrophe (U+2019),
so the fixture and `authorize/refused.rs` do not agree byte for byte; use a
straight apostrophe. And the comment in
`frontend/packages/apps/web/vite.config.ts`, "The header that server admits a
caller by", has a demonstrative with no antecedent; make it "The header the
coffret server admits a caller by."

No behaviour changes: the same requests are admitted and refused, the header on
the wire is the same, and the E2E stage asserts the same things — it only keeps
the server's explanation when an assertion fails.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] One name for the header on both sides, with the literal unchanged:
      `! grep -rqw CAPABILITY_HEADER backend/crates`,
      `! grep -rqw KEY_HEADER frontend/packages`,
      `grep -rqw SERVER_KEY_HEADER backend/crates`,
      `grep -rqw SERVER_KEY_HEADER frontend/packages`; `make check` proves no
      reference was left behind.
- [x] The API stage keeps a refusal's explanation:
      `test "$(grep -c -- --fail-with-body scripts/e2e-it.sh)" -ge 4`, with the
      script still parsing — `bash -n scripts/e2e-it.sh`. The CI `e2e` job runs
      the script itself.
- [x] The two text defects are gone:
      `! grep -q "device’s" frontend/packages/gateway/api/src/refusal.test.ts`
      and `! grep -q "that server admits" frontend/packages/apps/web/vite.config.ts`.
- [x] Existing backend, frontend and interoperability checks continue to pass.

## Out of scope

The header's value on the wire does not change. The MinIO health polls keep
`--fail`. The curly apostrophes in `FileList.tsx`, `newFolder.test.ts` and
`indexSnapshot.test.ts` are ordinary prose quoting nothing and are left alone.
What the browser stage's journeys assert does not change.
