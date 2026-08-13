# Brawler

Brawler is a server-authoritative top-down arena shooter. Milestone 01 establishes the smallest Bevy/Rust application foundation: a blank macOS client, a headless dedicated server, shared fixed-tick gameplay composition, and a Lightyear protocol registry with no network connections yet.

## Toolchain

The repository pins Rust 1.95.0 in [`rust-toolchain.toml`](rust-toolchain.toml). Bevy is pinned to 0.19.1 and Lightyear to 0.29.0. `Cargo.lock` is committed and must be updated intentionally.

## Canonical commands

Run these from the repository root:

```sh
just
```

`just` builds the isolated server and client configurations, launches both through Cargo's target resolution, and shuts down the other process when you press Ctrl-C, close the client window, or one process exits. If the server exits with a failure status, `just` stops the client and returns that status. The individual commands remain available for focused checks:

```sh
cargo fmt --all -- --check
cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings
cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings
cargo test --locked --no-default-features --features client --all-targets
cargo test --locked --no-default-features --features server --all-targets
cargo build --locked --no-default-features --features client --bin brawler-client
cargo build --locked --no-default-features --features server --bin brawler-server
cargo run --locked --no-default-features --features client --bin brawler-client
cargo run --locked --no-default-features --features server --bin brawler-server
./scripts/check-server-features.sh
```

The server command is headless and exits cleanly with Ctrl-C. The client opens a blank responsive window and exits cleanly when its window closes. `RUST_LOG` controls log filtering, for example `RUST_LOG=brawler=info`.

Do not use `--all-features` as a supported application build: client and server are independently tested feature configurations, and Cargo features are additive.

## Repository conventions

Future authored data and runtime assets will be added under a documented milestone once they have a real consumer. Until then, no empty asset/map/content directories are created. Third-party art, audio, fonts, and code must record provenance and license information alongside the content when introduced.
