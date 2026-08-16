set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default: help

# List the supported development commands.
help:
    @just --list

# Format all Rust sources.
fmt:
    cargo fmt --all

# Check formatting without changing files.
fmt-check:
    cargo fmt --all -- --check

# Check the isolated client configuration.
check-client:
    cargo check --locked --no-default-features --features client --all-targets

# Check the isolated dedicated-server configuration.
check-server:
    cargo check --locked --no-default-features --features server --all-targets

# Check the Crossbeam integration-test configuration.
check-network:
    cargo check --locked --no-default-features --features network-test --tests

check: check-client check-server check-network

# Build both production roles.
build: build-client build-server

build-client:
    cargo build --locked --no-default-features --features client --bin brawler-client

build-server:
    cargo build --locked --no-default-features --features server --bin brawler-server

# Run the isolated client unit and target tests.
test-client:
    cargo test --locked --no-default-features --features client --all-targets

# Run the isolated dedicated-server unit and target tests.
test-server:
    cargo test --locked --no-default-features --features server --all-targets

# Run deterministic Crossbeam and loopback-UDP network tests.
test-network:
    cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1

# Measure the fixed-tick budget with 100 headless fighters and 200 active projectiles.
test-performance:
    cargo test --locked --no-default-features --features network-test --test performance -- --nocapture

test: test-client test-server test-network test-performance

# Run Clippy for both independently buildable roles.
clippy: clippy-client clippy-server

clippy-client:
    cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings

clippy-server:
    cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings

lint: fmt-check clippy

# Verify all automated development-cycle gates, including the supervised UDP smoke.
verify: lint test server-features network-smoke

ci: verify

server-features:
    ./scripts/check-server-features.sh

# Generate API documentation for the client-facing library configuration.
docs:
    cargo doc --locked --no-deps --no-default-features --features client

# Remove Cargo build artifacts. This does not modify source files or Cargo.lock.
clean:
    cargo clean

# Preview removal of development and test artifacts while preserving release builds.
clean-debug-preview:
    cargo clean --profile dev --dry-run
    cargo clean --profile test --dry-run

# Remove development and test artifacts while preserving release builds.
clean-debug:
    cargo clean --profile dev
    cargo clean --profile test

# Run automated verification, then start the end-of-cycle interactive user test.
user-test: verify
    @printf '%s\n' 'Automated verification passed. Starting the two-client interactive user test.'
    BRAWLER_NETWORK_HEADLESS=0 ./scripts/network.sh

# Build both roles, then run one dedicated server and one client together.
run:
    #!/usr/bin/env bash
    set -euo pipefail

    cargo build --locked --no-default-features --features server --bin brawler-server
    cargo build --locked --no-default-features --features client --bin brawler-client

    server_pid=""
    client_pid=""

    job_is_running() {
        jobs -pr | grep -qx "$1"
    }

    # `cargo run` is a wrapper around the actual binary.  Signal the complete
    # descendant tree so Ctrl-C cannot leave the server binary orphaned.
    terminate_process_tree() {
        local pid="$1"
        local signal="$2"
        local child

        [[ -n "$pid" ]] || return 0
        kill -0 "$pid" 2>/dev/null || return 0
        while read -r child; do
            [[ -n "$child" ]] || continue
            terminate_process_tree "$child" "$signal"
        done < <(pgrep -P "$pid" 2>/dev/null || true)
        kill -"$signal" "$pid" 2>/dev/null || true
    }

    cleanup() {
        local status=$?
        # Background jobs inherit the launcher's ignored SIGINT disposition on
        # some shells.  TERM is therefore the reliable shutdown signal after
        # the launcher has already handled Ctrl-C.
        local signal=TERM

        terminate_process_tree "$client_pid" "$signal"
        terminate_process_tree "$server_pid" "$signal"
        wait "$client_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
        exit "$status"
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM

    # Run through Cargo so CARGO_TARGET_DIR and Cargo configuration determine
    # the executable location instead of assuming target/debug in the repo.
    (trap - INT TERM; exec cargo run --locked --no-default-features --features server --bin brawler-server) &
    server_pid=$!
    (trap - INT TERM; exec cargo run --locked --no-default-features --features client --bin brawler-client -- --client-id 1) &
    client_pid=$!

    while :; do
        # jobs -pr distinguishes completed background jobs from live ones;
        # kill -0 alone can still report a completed, unreaped child as alive.
        if ! job_is_running "$server_pid"; then
            if wait "$server_pid"; then
                server_status=0
            else
                server_status=$?
            fi
            printf 'brawler launcher: server exited with status %s; stopping client\n' "$server_status" >&2
            kill -TERM "$client_pid" 2>/dev/null || true
            wait "$client_pid" 2>/dev/null || true
            exit "$server_status"
        fi

        if ! job_is_running "$client_pid"; then
            if wait "$client_pid"; then
                client_status=0
            else
                client_status=$?
            fi
            exit "$client_status"
        fi

        sleep 0.1
    done

# Launch one server and two distinguishable windowed clients.
network:
    BRAWLER_NETWORK_HEADLESS=0 ./scripts/network.sh

# Launch one server and two windowed clients in Hot Zone mode (BRAWLER_NETWORK_MATCH_RULES=verification for a 30-tick capture target).
network-hot-zone:
    BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_GAME_MODE=hot-zone ./scripts/network.sh

# Launch two windowed clients with client 1 firing at the neutral dummy.
network-combat:
    BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1 ./scripts/network.sh

# Launch two windowed clients with client 1 using the native gamepad path.
network-controller:
    BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_CONTROLLER_DEMO=1 ./scripts/network.sh

# Repeat the windowed combat scenario using the 30 Hz presentation profile.
network-combat-30:
    BRAWLER_RENDER_PROFILE=30 BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1 ./scripts/network.sh

# Repeat the windowed combat scenario using the 60 Hz presentation profile.
network-combat-60:
    BRAWLER_RENDER_PROFILE=60 BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1 ./scripts/network.sh

# Repeat the windowed combat scenario using the high-refresh/no-vsync profile.
network-combat-high:
    BRAWLER_RENDER_PROFILE=high BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1 ./scripts/network.sh

# Launch the combat demo with an explicit preset (1 Pulse, 2 Scatter, 3 Arc, 4 Blade).
network-combat-pulse:
    BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1 BRAWLER_NETWORK_WEAPON_PRESET=1 ./scripts/network.sh

network-combat-scatter:
    BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1 BRAWLER_NETWORK_WEAPON_PRESET=2 ./scripts/network.sh

network-combat-arc:
    BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1 BRAWLER_NETWORK_WEAPON_PRESET=3 ./scripts/network.sh

network-combat-blade:
    BRAWLER_NETWORK_HEADLESS=0 BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1 BRAWLER_NETWORK_WEAPON_PRESET=4 ./scripts/network.sh

# Launch bounded headless clients; succeeds only after server movement/facing assertions.
network-smoke:
    BRAWLER_NETWORK_HEADLESS=1 ./scripts/network.sh

# Run repeated local, typical, and adverse combat convergence profiles.
network-combat-profiles:
    ./scripts/network-combat-profiles.sh
