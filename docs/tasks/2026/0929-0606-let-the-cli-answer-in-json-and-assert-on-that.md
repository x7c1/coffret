---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, user-experience]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && ! git grep -qE "sed -n .s\|\^On Storage|said .\^added |said .\^fetched |\"committed head \"|summary##\*committed head|sed -n .s/\^fetched " -- scripts/'
assignee: null
branch: task/0929-0606-let-the-cli-answer-in-json-and-assert-on-that
created_at: 2026-09-29T16:03:58Z
updated_at: 2026-09-29T17:25:36Z
---

# feat(cli): answer in JSON with `--json`, and have the hardware targets assert on it

## Overview

The real-hardware targets (`scripts/drive-round-trip-it.sh`,
`scripts/drive-index-layout-it.sh`) and `scripts/e2e-it.sh` read the facts they
assert on out of the CLI's human-readable output with `grep` / `sed`. Changing a
sentence meant for a person therefore breaks a target, and the real-Drive
targets only run with a person's OAuth consent, so the break is found late. The
CLI has no machine-readable output today (`coffret-cli` has no serde; the
outcome types `CreatedLibrary`, `SyncOutcome`, `FetchOutcome` are
`#[derive(Debug)]` only).

Add a machine-readable answer to the CLI and move the scripts' assertions on
*facts* onto it.

**The CLI side.**

- A global `--json` flag. With it, a command prints exactly one JSON object on
  stdout when it finishes — success or failure — and nothing else on stdout;
  progress, advice and log-location lines stay on stderr. Without it, output is
  byte-for-byte what it is today.
- The facts the scripts need, per command (field names are the implementer's
  choice; keep them in the repository's vocabulary — *Entry*, *Library*,
  *finding*, *head*, …):
  - `init` / `join`: where the Library is on Storage (S3 bucket + prefix, or the
    Drive folder id), the Library name, and for `init` / `recovery-code` the
    Recovery Code; for `join`, whether Storage held nothing of the Library yet,
    and whether a consent flow was started.
  - `sync`: added / replaced / unchanged counts, the committed head generation or
    that nothing was committed, and the findings (path + a stable reason kind).
  - `freeze`: packs, entries held, absorbed, packed already, committed head.
  - `fetch`: fetched / containers / skipped counts (and the single-Entry form).
  - `mappings`: the list of prefix + local root.
  - Every command: the path of the run's log file, which scripts now take from
    the `Logging this run to …` stderr line.
  - On failure: a stable error **kind** (reuse the kinds the server already puts
    on the wire where one exists, e.g. for refusals; otherwise name the device
    error's variant), plus the same sentence the text mode prints. Kinds the
    scripts branch on today: no usable grant / credentials rejected, Index
    schema version too old (with the found and supported versions), a mapping
    that cannot be opened.
- Serialize from dedicated output types in `coffret-cli` (or a small module the
  CLI owns), not by deriving `Serialize` on domain types, so the JSON shape is a
  CLI contract that can be kept stable independently of internal refactors.
  Document the shape in the CLI's `--help` for `--json` and pin it with golden
  tests per command.
- Secrets: the Recovery Code appears in JSON only where the text mode already
  prints it on stdout (`init`, `recovery-code`). No passphrase, token, or local
  path beyond what the text mode prints may appear (spec: EL-1 applies to what
  is written down, and stdout JSON is a response, like the text mode).

**The scripts side.**

- Replace every assertion on a *fact* with a `jq` read of the JSON answer —
  including at least: `On Storage:` / `^coffret1` parsing of `init`, the `added …
  committed head …` summary of `sync`, the `fetched …, containers …` summary of
  `fetch`, `^surfaced …` finding checks, `Logging this run to …`, the dead-grant
  detection, and the schema-version refusal.
- Keep text assertions only where the **sentence itself** is what is under test
  — e.g. that a refusal names the command to run next (`coffret authorize`,
  `coffret map`) — and give each kept one a one-line comment saying so.
- `jq` becomes a required tool of these scripts; check for it up front the way
  the scripts already check for their other tools.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `--json` golden tests pin the JSON answer of `init`, `join`,
      `recovery-code`, `sync`, `freeze`, `fetch` and `mappings` on success, and
      of at least the no-usable-grant, schema-version and unopenable-mapping
      failures.
- [x] A test pins that without `--json` the text output of `sync` and `fetch` is
      unchanged.
- [x] A test pins that stdout under `--json` holds exactly one JSON object (no
      progress lines, no advice) for a command that also prints progress.
- [x] No script parses `On Storage:`, the `sync` summary, or the `fetch` summary
      out of text any more (grep gate appended to `check_command`).

### Manual / on-hardware (verified by a human before merge)

- [ ] CI's `e2e` and `s3-store` jobs are green on the PR.
- [ ] `make drive-round-trip-it` and `make drive-index-layout-it` (the latter
      refuses to run while the schema versions are equal — its refusal is the
      expected outcome) are green against real Drive with the JSON assertions.
