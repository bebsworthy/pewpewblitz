# Brawler

Brawler is a server-authoritative top-down arena shooter. Milestone 02 adds a Lightyear Netcode/UDP connection and replication sandbox: two local clients can join one headless server and observe its stable server-owned placeholder roster.

## Toolchain

The repository pins Rust 1.95.0 in [`rust-toolchain.toml`](rust-toolchain.toml). Bevy is pinned to 0.19.1 and Lightyear to 0.29.0. `Cargo.lock` is committed and must be updated intentionally.

## Canonical commands

Run these from the repository root:

```sh
just
```

`just` retains the Milestone 01 one-client launcher. For the network sandbox, use `just network` to open one server and two distinguishable client windows, or `just network-smoke` for a bounded headless process check. The launcher runs through Cargo's target resolution, supervises every child, propagates failures, and leaves no Brawler processes after shutdown. The individual commands remain available for focused checks:

```sh
cargo fmt --all -- --check
cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings
cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings
cargo test --locked --no-default-features --features client --all-targets
cargo test --locked --no-default-features --features server --all-targets
cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1
cargo build --locked --no-default-features --features client --bin brawler-client
cargo build --locked --no-default-features --features server --bin brawler-server
cargo run --locked --no-default-features --features client --bin brawler-client -- --client-id 1
cargo run --locked --no-default-features --features server --bin brawler-server -- --bind 127.0.0.1:5000
./scripts/check-server-features.sh
just network
just network-smoke
```

The server accepts `--bind`, `--max-clients`, and `--handshake-timeout-ms`. The client accepts `--server`, required `--client-id`, and bounded automation flags `--headless --exit-after-roster 2`. `RUST_LOG` controls log filtering, for example `RUST_LOG=brawler=info`. Window titles identify the two clients; structured logs report connection outcome and stable `(player_id, network_entity_id)` roster entries.

The network launcher waits for its own server's readiness signal before starting clients, so a bind
collision cannot accidentally test a pre-existing server. It remains running when one windowed
client closes so the remaining roster can be observed. Restart that client with the same individual
command and `--client-id`; set `BRAWLER_NETWORK_TIMEOUT_SECONDS` to add a bounded windowed-session
deadline when needed.

Do not use `--all-features` as a supported application build: client and server are independently tested production configurations, while `network-test` is the dedicated separate-app Crossbeam integration configuration. Cargo features are additive.

## Repository conventions

Future authored data and runtime assets will be added under a documented milestone once they have a real consumer. Until then, no empty asset/map/content directories are created. Third-party art, audio, fonts, and code must record provenance and license information alongside the content when introduced.
