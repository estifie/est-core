# est-core

The shared foundation of the EST product ecosystem: the command-line
contract, layered path resolution, and in-memory secret handling that
every EST product builds on. Dependency-free by policy.

Status: pre-release. The API is unstable until 1.0; expect small,
documented breaks between 0.x versions. Platform: macOS in v1
(`store` file modes are Unix-only).

## Use

Not yet published to crates.io. Until the first release, depend on it
by git:

```toml
est-core = { git = "https://github.com/estifie/est-core", branch = "main" }
```

```rust
use est_core::{cli, secret};

fn main() {
    let token = secret::Secret::new(std::env::var("TOKEN").unwrap());
    println!("{}", cli::ok(&format!("\"len\":{}", token.expose().len())));
}
```

## Modules

| Module | Purpose |
|---|---|
| `cli` | Exit codes, versioned JSON envelope, string escaper, destructive-command rule |
| `config` | `[core]` settings loader with warnings, duration syntax |
| `log` | `EST_LOG` levels, structured stderr records |
| `paths` | Layered base resolution, product config dirs, pointer files |
| `process` | Supervised spawn with timeout, output cap, combined log |
| `secret` | `Secret<T>` redaction wrapper, log scrubbing, reveal audit log |
| `store` | Atomic writes, mode-correct dirs, file locks, retention |

Full specs: [CLI contract v1](docs/cli-contract-v1.md),
[port registry](docs/ports.md).

## Develop

```sh
git config core.hooksPath .githooks   # one-time: enable the repo hooks
cargo fmt --all -- --check            # gate 1: formatting
cargo clippy --all-targets -- -D warnings   # gate 2: lints
cargo test                            # gate 3: unit + integration + doc tests
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow,
[SECURITY.md](SECURITY.md) for reporting, and
[CHANGELOG.md](CHANGELOG.md) for what changed.

## License

Dual-licensed under MIT or Apache-2.0, at your option. See
[LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
