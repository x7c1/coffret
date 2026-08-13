# coffret

A personal E2EE file library: encrypt your folders on your machine, store only ciphertext in your own cloud storage, and browse everything locally.

## Layout

- `backend/` — Rust `cargo` workspace. Crates live under `crates/` and are
  split by architectural layer: `apps/` (bins), `gateway/` (external I/O),
  `domain/` (model + usecase), `libs/` (shared).
- `frontend/` — TypeScript pnpm workspace. Packages live under `packages/`
  and are split by layer: `apps/`, `ui/`, `gateway/`, `domain/`.

## Documentation

- [docs/concepts/](docs/concepts/) — the domain vocabulary of the product

## Development

Requires stable Rust and Node 24. The pnpm version is pinned by the
`packageManager` field in `frontend/package.json`.

Run `make help` for the full target list. `make check` runs the full pre-PR
gate (backend fmt/build/test/clippy + frontend build/typecheck/test/lint).

## License

[AGPL-3.0-only](LICENSE)

Contributions are accepted under the terms described in [CLA.md](CLA.md).
