---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && bash -n scripts/e2e-it.sh scripts/drive-round-trip-it.sh'
assignee: null
branch: task/0908-1929-read-recovery-codes-without-command-line-arguments
created_at: 2026-09-08T19:29:38Z
updated_at: "2026-09-09T11:28:11Z"
---

# fix(cli): read recovery codes without command-line arguments

## Overview

`coffret join --recovery-code VALUE` exposes the Recovery Code in process
arguments and encourages leaving it in shell history. A Recovery Code carries
the Master Key, so join must obtain it through a non-echoing terminal prompt
instead of a value-bearing command-line argument.

Remove the value-bearing option. Add an explicit `--recovery-code-stdin` mode
for scripts, reading one bounded line without echoing it. When both this and
`--passphrase-stdin` are selected, the input order is Recovery Code on the first
line, then this device's Passphrase on the next line; document that order in
help and usage documentation. The normal interactive path asks for the Recovery
Code without echo and still asks for this device's new Passphrase twice.
Reuse the shell crate's existing secret-input boundary where practical. Keep
secret input short-lived, and never echo the submitted value in errors, logs,
command transcripts, or a successful join report.

Do not make malformed provider arguments consume secret input first. Missing
input, EOF, an overlong line, and an invalid code must fail clearly without
printing the code. Tests must run without a person's terminal or live provider.
An explicit stdin mode is required for unattended use: do not silently consume
redirected stdin as though the user had selected it. Keep root and provider
validation behavior coherent with the existing join flow.

Update active shell scripts, CLI integration tests, and current usage docs that
pass the old argument. In `scripts/e2e-it.sh` and `scripts/drive-round-trip-it.sh`,
feed the code through stdin without adding it to logged command strings. Keep
the automated new-device round trip usable. Historical completed task files
remain unchanged.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] CLI parsing and help tests reject the old value-bearing option, expose
      the explicit stdin switch, and state the two-line order when both secrets
      come from stdin.
- [x] Tests prove that separate Recovery Code and Passphrase lines reach their
      respective consumers, including EOF, invalid input, and an overlong code
      without leaking the supplied secret into output or errors.
- [x] Interactive-input tests exercise the non-echoing prompt boundary and
      preserve the twice-chosen Passphrase behavior without requiring a human.
- [x] Tests prove that malformed provider arguments fail before requesting or
      consuming secret input.
- [x] Existing CLI setup tests and the build cover the updated callers; shell
      script syntax checks and regression assertions protect the scripts from
      placing the Recovery Code in argv or their command transcript.

## Out of scope

Recovery Code encoding, cryptography, provider grants, storage format, and
root-marker registration do not change. Do not redesign general command output
or refactor unrelated error modules. Real-provider authorization remains a
separate environment-dependent check.
