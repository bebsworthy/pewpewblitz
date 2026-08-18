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

# Check the engine-independent routed-server contract package.
check-routing:
    cargo check --locked -p brawler-routing --all-targets

check: check-routing check-client check-server check-network

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

# Run deterministic routed codec, queue, and memory-backend tests.
test-routing:
    cargo test --locked -p brawler-routing

# Measure the fixed-tick budget with 100 headless fighters and 200 active projectiles.
test-performance:
    cargo test --locked --no-default-features --features network-test --test performance -- --nocapture

test: test-routing test-client test-server test-network test-performance

# Run Clippy for every independently buildable role.
clippy: clippy-routing clippy-client clippy-server

clippy-routing:
    cargo clippy --locked -p brawler-routing --all-targets -- -D warnings

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
    BRAWLER_NETWORK_HEADLESS=0 ./scripts/network-routed.sh

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
    BRAWLER_NETWORK_HEADLESS=0 ./scripts/network-routed.sh

# Run the headless routed lobby-to-match process smoke.
network-routed-smoke:
    BRAWLER_NETWORK_HEADLESS=1 ./scripts/network-routed.sh

# Run the same production routed smoke over the IPv6 loopback address. The client derives an IPv6
# local socket from the selected server address; this is a separate opt-in check because many
# development hosts disable IPv6 loopback.
network-routed-ipv6-smoke:
    BRAWLER_NETWORK_HEADLESS=1 BRAWLER_ROUTED_BIND="[::1]:5000" ./scripts/network-routed.sh

# Capture the optional macOS routed MTU evidence on lo0, then parse it without third-party Python
# packages. BPF capture permission is host-controlled; failure keeps the M01 capture gate
# unsupported. Override the output path with `just network-routed-capture capture=...`.
network-routed-capture capture="target/routed-capture.pcap":
    ./scripts/network-routed-capture.sh {{capture}}

# Parse an already collected classic pcap. The verifier passes only with observed IPv4/IPv6 UDP
# traffic at or below 1,200-byte payloads and no IPv4 fragmentation or IPv6 Fragment headers.
verify-routed-capture capture:
    python3 scripts/verify-routed-capture.py --json {{capture}}

# Run bounded routed process/lifecycle evidence and write a machine-readable summary. `mode=both`
# runs the requested cycles per mode; `mode=crash-restart` delegates to production-worker process
# tests that assert exact terminal child/route/queue/socket cleanup. Unsupported paired
# full IPC latency, stop duration, paired CPU/bandwidth, and MTU gates are reported unsupported.
network-routed-evidence cycles="5" timeout="90" mode="wipeout":
    python3 scripts/network-routed-evidence.py --cycles {{cycles}} --timeout {{timeout}} --mode {{mode}}

# Run paired direct-UDP/routed measurements. `pairs=1` is the bounded local smoke; the milestone
# gate uses three sequential pairs. Direct transport bytes and routed inner bytes are compared
# directionally; public-envelope and mixed-control IPC overhead stay diagnostic-only.
network-paired-evidence pairs="3" timeout="90" mode="wipeout":
    python3 scripts/network-paired-evidence.py --pairs {{pairs}} --timeout {{timeout}} --mode {{mode}}

# Run parser/gate unit tests without launching network processes.
test-paired-evidence:
    python3 -m unittest scripts/test_network_paired_evidence.py

# Preserve the completed v1 direct-UDP topology as M01's explicit comparison baseline.
network-direct:
    BRAWLER_NETWORK_HEADLESS=0 ./scripts/network.sh

# Run the headless direct-UDP comparison baseline with its existing authority checks.
network-direct-smoke:
    BRAWLER_NETWORK_HEADLESS=1 ./scripts/network.sh

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
    BRAWLER_NETWORK_HEADLESS=1 ./scripts/network-routed.sh

# Run one headless terrain-destruction profile (Wipeout) with Arc Launcher clients.
network-terrain:
    BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ASSERT_TERRAIN=1 \
    BRAWLER_NETWORK_WEAPON_PRESET=3 BRAWLER_NETWORK_WEAPON_PRESET_CLIENT_TWO=3 \
    ./scripts/network.sh

# Run the same terrain profile under Hot Zone rules around the central objective.
network-terrain-hot-zone:
    BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ASSERT_TERRAIN=1 \
    BRAWLER_NETWORK_GAME_MODE=hot-zone \
    BRAWLER_NETWORK_WEAPON_PRESET=3 BRAWLER_NETWORK_WEAPON_PRESET_CLIENT_TWO=3 \
    ./scripts/network.sh

# Run repeated local, typical, and adverse combat convergence profiles.
network-combat-profiles:
    ./scripts/network-combat-profiles.sh

# Run the deterministic repeated-match and reconnect soak scenarios (25 matches per mode, 20 reconnect cycles).
soak:
    cargo test --locked --no-default-features --features network-test --test network soaks -- --test-threads=1 --nocapture

# Run one closeout-instrumented Wipeout smoke; reports land under target/diagnostics/<scenario>.
closeout-wipeout:
    #!/usr/bin/env bash
    set -euo pipefail
    out="target/diagnostics/wipeout-$(date +%Y%m%d-%H%M%S)"
    BRAWLER_NETWORK_HEADLESS=1 BRAWLER_DIAGNOSTICS_DIR="$out" BRAWLER_DIAGNOSTICS_SCENARIO_ID=m11-wipeout-closeout ./scripts/network.sh
    printf 'closeout reports: %s\n' "$out"

# Run one closeout-instrumented Hot Zone smoke with the same validation.
closeout-hot-zone:
    #!/usr/bin/env bash
    set -euo pipefail
    out="target/diagnostics/hot-zone-$(date +%Y%m%d-%H%M%S)"
    BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_GAME_MODE=hot-zone \
    BRAWLER_DIAGNOSTICS_DIR="$out" BRAWLER_DIAGNOSTICS_SCENARIO_ID=m11-hot-zone-closeout \
    ./scripts/network.sh
    printf 'closeout reports: %s\n' "$out"

# Build the dedicated server with process-global Lightyear metrics for measurement runs.
build-server-metrics:
    cargo build --locked --no-default-features --features "server,process-metrics" --bin brawler-server

# Run the M03 owner-prediction comparison matrix (experimental feature build).
prediction-comparison:
    cargo test --locked --no-default-features --features "network-test,owner-prediction" --test network prediction -- --test-threads=1 --nocapture
