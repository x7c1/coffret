# Claude AI Guidelines

## Repository layout

Coffret has two top-level parts, treated as equals:

- `backend/` — Rust `cargo` workspace. Crates live under `crates/` and are
  split by architectural layer: `apps/` (bins), `gateway/` (external I/O),
  `domain/` (model + usecase), `libs/` (shared). Dependency direction is
  enforced by crate boundaries.
- `frontend/` — TypeScript pnpm workspace. Packages live under `packages/`
  and are split by layer: `apps/`, `ui/`, `gateway/`, `domain/`.

## Code Quality

After changing code, run `make check` (or the backend/frontend half you
touched) and fix any issues before considering the task complete.

### Fix issues as you find them

- When you notice code smells or inappropriate patterns (silent error
  suppression, missing logs, inconsistent naming, etc.) during implementation,
  fix them in the same PR. Do not leave them for later or require the user to
  point them out.
- Do not defer cleanup of code you just wrote to a future PR. Reserve "out of
  scope" for genuinely unrelated large-scale refactors, not for polish on your
  own changes.

### Error types

- Error enums do not derive `PartialEq` / `Eq`. Tests assert by matching the
  variant and destructuring its fields — never by comparing whole error
  values, and never through their `Debug` or `Display` output. Deriving
  equality made every field addition a breaking change and kept variants from
  carrying dynamic information such as a source error or a path.

### Tests over manual verification

- When a behaviour could be checked by hand against a live provider, prefer
  an automated test that drives the real adapter through a scripted transport
  (see the Google Drive retry tests). Hand-run checks are not reproducible
  and silently rot; only keep one when no scripted equivalent exists.

## Language

Documentation, code comments, commit messages, and pull-request descriptions
are written in English.

## Commit and PR messages

Commit messages and pull-request descriptions must be self-contained. Do not
reference external planning labels (milestone or sub-plan identifiers) or any
private/internal repository or document.

## Git

- Do not commit directly to main — always create a branch and open a pull
  request.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `docs:`, …)
  with an optional scope, e.g. `feat(backend): add upload queue`.
