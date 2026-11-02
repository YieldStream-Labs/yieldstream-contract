# Contributing to YieldStream

Thanks for your interest in improving YieldStream — a multi-tenant Merkle
distributor for Soroban on Stellar.

## Before you start

- Pick an issue and leave a comment asking to be assigned **before** opening a
  PR. Issues labeled `good-first-issue` are a great starting point.
- If your idea isn't tracked yet, open an issue first and wait for maintainer
  feedback before building.

## Development setup

1. Install a current stable Rust toolchain (`rustup`). The pinned targets
   (`wasm32v1-none`, `wasm32-unknown-unknown`) are installed automatically from
   `rust-toolchain.toml`.
2. Install the Stellar CLI: https://stellar.org/developers
3. Build and test:

   ```bash
   cargo test --workspace                          # native test suite (soroban-sdk testutils)
   cargo build --release --target wasm32-unknown-unknown -p yieldstream-distributor
   cargo fmt --all
   cargo clippy --all-targets -- -D warnings
   ```

## Pull request guidelines

- One logical change per PR. Link the issue it closes (`Closes #123`).
- **Every behavior change needs a test.** We keep a 1:1 discipline of feature →
  test function in `contracts/distributor/src/test/`.
- Merkle hashing changes must keep the repo RFC 6962-compliant (domain
  separation `0x00`/`0x01`, sorted sibling pairs) and must not break the
  canonical vectors in `test-vectors/merkle_v1.json`. If a change legitimately
  alters vector semantics, regenerate them via the vector tests and call it
  out explicitly in the PR description.
- CI (`cargo fmt`, `clippy -D warnings`, tests, WASM build) must pass.
- Keep storage-footprint in mind: we extend TTLs deliberately, don't add
  persistent entries without justification.

## Code style

- `cargo fmt` output, no exceptions.
- Errors: add variants to `src/errors.rs` with a `#[notoc]`-friendly doc
  comment; never `panic!` in contract code — return `Error`.
- Public entrypoints live in `src/lib.rs` and stay thin: validate → storage
  op → event. Logic belongs in `merkle.rs` / `storage.rs`.

## Communication

- Bug reports and feature requests: GitHub issues.
- Security issues: see [SECURITY.md](SECURITY.md). Do **not** open a public
  issue for vulnerabilities.

## License

By contributing, you agree that your contributions will be licensed under both
the MIT License (`LICENSE-MIT`) and Apache License 2.0 (`LICENSE-APACHE`), at
the recipient's option.
