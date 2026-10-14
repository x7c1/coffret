---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1015-0357-update-the-frontend-dev-dependencies-that-carry-advisories-and-audit-them-in-ci
created_at: 2026-10-14T18:57:00Z
updated_at: 2026-10-14T19:51:53Z
---

# fix(deps): update the frontend dev dependencies that carry advisories, and audit them in CI

## Overview

`pnpm audit --prod` reports nothing. The full `pnpm audit` (in `frontend/`)
reported 17 advisory entries on 2026-10-14 (13 distinct advisories: Critical
3, High 7, Moderate 7 by entry), all in development and test dependencies:

- `vitest 2.1.9` and what it pulls in: `vite 5.4.21`, `esbuild 0.21.5`,
  `@vitest/mocker`, `tinypool 1.1.1`. The root `package.json` asks for
  `vitest ^2.1.8`, while `@coffret/web` already uses `vite ^6`.
- lint and build: `js-yaml 4.3.1`, `brace-expansion 1.1.18 / 5.0.9`,
  `source-map-js 1.2.1`.

None of these ships in the explorer the server and the desktop app embed, and
the normal scripts run `vitest run` without its UI or API server. So this is
not an exploit in the product, but the test toolchain runs on the same device
as a person's Libraries and should not carry known advisories.

1. Update the test, build and lint toolchain so that `pnpm audit` is clean, or
   as clean as the upstreams allow. Move `vitest` to a current major and fix
   what the major bump breaks (configuration, APIs, snapshot or mock
   behaviour) without weakening a test. Do not take the minimum version the
   audit names on trust. Check each advisory's own fixed range, because the
   audit's suggestion and the primary advisory sometimes disagree.
2. For any advisory that cannot be cleared (no fixed release yet, or a fix
   only in a major that cannot be adopted now), add a reasoned, scoped
   allowance in pnpm's own configuration. The reason states the input path and
   the mode it would need (for example: "needs vitest's UI server exposed") and
   why that does not hold here. Nothing is allowed without a reason.
3. Add a dependency audit to CI's `frontend` job that fails on an advisory
   not covered by such an allowance, and give it a `make` target the way
   `make deny` stands for `cargo deny`. Say in the workflow comment, as the
   `cargo-deny` job does, why it can turn red with this repository unchanged.
   Decide whether `make check` should run it. `make check` today needs no
   network beyond installing dependencies; follow the reasoning `make deny`
   follows.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The frontend builds, typechecks, tests and lints with the updated
      toolchain, and no test was removed or loosened to get there
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] `pnpm audit` in `frontend/` reports no advisory outside the reasoned
      allowances, and the PR lists each allowance with its reason
- [x] The new CI audit step ran on the PR and passed
