# Contributing

## Setup

Rust 1.89 or newer. `rust-toolchain.toml` pins the exact toolchain;
rustup picks it up automatically.

```sh
git config core.hooksPath .githooks   # pre-commit: fmt; pre-push: clippy + tests
cargo test                            # everything should pass before you start
```

## Gates

CI runs the same gates on every push and pull request:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps                   # with RUSTDOCFLAGS="-D warnings" in CI
```

`cargo deny check` and `cargo audit` also run in CI. This crate is
dependency-free by policy: a new dependency needs justification in
the PR and must pass the `deny.toml` review (MIT/Apache-2.0 only).

## Disk hygiene

All EST checkouts in one folder share the sibling `.target` directory
(see `.cargo/config.toml`), so dependencies compile once. Plain
`cargo clean` wipes the shared dir — safe, everything rebuilds; use
`cargo clean -p <pkg>` to prune a single crate.

## Commits and PRs

- Conventional Commits: `feat:`, `fix:`, `docs:`, `refactor:`,
  `test:`, `chore:`. Scope when it helps (`feat(cli): …`).
- One logical change per commit; `main` stays releasable.
- PRs squash-merge with a changelog-worthy title and add a
  `CHANGELOG.md` entry under `[Unreleased]`.
- Public API needs doc comments (`missing_docs` warns locally and
  fails CI) and tests that pin the behavior.
