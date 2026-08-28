set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# List the everyday development commands.
help:
    @just --list

# Run the routed supervisor and production lobby on localhost.
server:
    ./scripts/dev.sh server

# Run the Practice Balance Lab topology with one interactive client.
balance-lab:
    ./scripts/dev.sh balance-lab

# Open one interactive product client against the local routed server.
client:
    ./scripts/dev.sh client

# Run the routed server and exactly N interactive clients.
run clients:
    ./scripts/dev.sh run {{clients}}

# Format all Rust sources.
fmt:
    cargo fmt --all

# Check every independently buildable role.
check: _check-routing _check-client _check-server _check-network _check-balance-lab

# Run formatting, Clippy, and dedicated-server isolation checks.
lint: _fmt-check _balance-lab-web _clippy-routing _clippy-client _clippy-server _clippy-network _clippy-balance-lab _server-features _v3-world-presentation _map-cleanup

# Run all deterministic Rust test suites, including the performance gates.
test: _test-routing _test-client _test-server _test-balance-lab _test-network _test-performance

# Run a real-process product match with 2, 4, or 6 clients (default: 2).
e2e clients="2":
    ./scripts/e2e.sh {{clients}}

# Run one exact real-process Practice game with one human and manifest bots.
practice-e2e game_type="wipeout-1v1":
    ./scripts/e2e-practice.sh {{game_type}}

# Record one bounded routed release-client render report at 1280x720.
v3-render-evidence report="target/v3-render-evidence.txt":
    ./scripts/v3-render-evidence.sh {{report}}

# Run the complete automated gate, including 2/4/6-client product matches and all Practice types.
ci: lint test _e2e-matrix _practice-e2e-matrix

# Remove Cargo build artifacts.
clean:
    cargo clean

_fmt-check:
    cargo fmt --all -- --check

_check-client:
    cargo check --locked --no-default-features --features client --all-targets

_check-server:
    cargo check --locked --no-default-features --features server --all-targets

_check-network:
    cargo check --locked --no-default-features --features network-test --tests

_check-routing:
    cargo check --locked -p brawler-routing --all-targets

_balance-lab-web:
    npm ci --prefix tools/balance-lab-web
    npm run --prefix tools/balance-lab-web test
    npm run --prefix tools/balance-lab-web build

_check-balance-lab: _balance-lab-web
    cargo check --locked --no-default-features --features balance-lab --all-targets

_clippy-routing:
    cargo clippy --locked -p brawler-routing --all-targets -- -D warnings

_clippy-client:
    cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings

_clippy-server:
    cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings

_clippy-network:
    cargo clippy --locked --no-default-features --features network-test --tests -- -D warnings

_clippy-balance-lab:
    cargo clippy --locked --no-default-features --features balance-lab --all-targets -- -D warnings

_server-features:
    ./scripts/check-server-features.sh

_v3-world-presentation:
    ./scripts/check-v3-world-presentation.sh

_map-cleanup:
    ./scripts/check-v8-map-cleanup.sh

_test-client:
    cargo test --locked --no-default-features --features client --all-targets

_test-server:
    cargo test --locked --no-default-features --features server --all-targets

_test-balance-lab:
    cargo test --locked --no-default-features --features balance-lab --all-targets
    cargo test --locked --no-default-features --features network-test,balance-lab --test network builds::revised_catalog_loadout_keeps_build_identity_and_replicates_authoritative_values -- --test-threads=1

_test-network:
    cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1

_test-routing:
    cargo test --locked -p brawler-routing

_test-performance:
    cargo test --locked --no-default-features --features network-test --test performance -- --nocapture

_e2e-matrix:
    ./scripts/e2e.sh 2
    ./scripts/e2e.sh 4
    ./scripts/e2e.sh 6

_practice-e2e-matrix:
    ./scripts/e2e-practice.sh wipeout-1v1
    ./scripts/e2e-practice.sh wipeout-2v2
    ./scripts/e2e-practice.sh wipeout-3v3
    ./scripts/e2e-practice.sh hot-zone-1v1
    ./scripts/e2e-practice.sh hot-zone-2v2
    ./scripts/e2e-practice.sh hot-zone-3v3
    ./scripts/e2e-practice.sh heist-1v1
    ./scripts/e2e-practice.sh heist-2v2
    ./scripts/e2e-practice.sh heist-3v3
