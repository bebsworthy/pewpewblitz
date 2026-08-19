#!/bin/sh
set -eu

# Canonical local v2 M01 launcher: one public supervisor, one lobby worker, one match worker after
# allocation, and two clients. The supervisor owns every worker and reaps them during cleanup.

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bind_addr=${BRAWLER_ROUTED_BIND:-127.0.0.1:5000}
headless=${BRAWLER_NETWORK_HEADLESS:-0}
timeout_seconds=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-90}
metrics_file=${BRAWLER_ROUTED_METRICS_FILE:-}
window_dir=${BRAWLER_ROUTED_WINDOW_DIR:-}
worker_features=${BRAWLER_ROUTED_SERVER_FEATURES:-server}
client_features=${BRAWLER_ROUTED_CLIENT_FEATURES:-client}
game_mode=${BRAWLER_ROUTED_GAME_MODE:-wipeout}
match_rules=${BRAWLER_ROUTED_MATCH_RULES:-}
simulation_ticks=${BRAWLER_ROUTED_SIMULATION_TICKS:-4000}
if [ -z "$match_rules" ]; then
    if [ "$headless" = 1 ]; then
        # The verification profile keeps the authoritative semantics while bounding the active
        # match deadline for this process lifecycle evidence run. Windowed play remains production.
        match_rules=verification
    else
        match_rules=production
    fi
fi
case "$match_rules" in
    production | verification) ;;
    *)
        echo "brawler routed network: match rules must be production or verification" >&2
        exit 2
        ;;
esac
case "$game_mode" in
    wipeout | hot-zone) ;;
    *)
        echo "brawler routed network: game mode must be wipeout or hot-zone" >&2
        exit 2
        ;;
esac
case "$timeout_seconds" in
    '' | *[!0-9]*)
        echo "brawler routed network: timeout must be a nonnegative integer" >&2
        exit 2
        ;;
esac
case "$simulation_ticks" in
    '' | *[!0-9]* | 0)
        echo "brawler routed network: simulation ticks must be a positive integer" >&2
        exit 2
        ;;
esac
supervisor_pid=
client_one_pid=
client_two_pid=
watchdog_pid=

cleanup() {
    trap - INT TERM EXIT
    if [ -n "$watchdog_pid" ] && kill -0 "$watchdog_pid" 2>/dev/null; then
        kill "$watchdog_pid" 2>/dev/null || true
    fi
    for pid in "$client_one_pid" "$client_two_pid" "$supervisor_pid"; do
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            kill -INT "$pid" 2>/dev/null || true
        fi
    done
    for pid in "$client_one_pid" "$client_two_pid" "$supervisor_pid"; do
        if [ -n "$pid" ]; then
            wait "$pid" 2>/dev/null || true
        fi
    done
}
trap cleanup INT TERM EXIT

cd "$project_dir"
cargo build --locked --bin brawler-client --no-default-features --features "$client_features"
cargo build --locked --bin brawler-server --no-default-features --features "$worker_features"
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$(target/debug/brawler-server routing-identity)
network_protocol=$(printf '%s\n' "$identity" | awk -F= '$1 == "network_protocol" { print $2 }')
registry_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }')
content_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "content_fingerprint" { print $2 }')
if [ -z "$network_protocol" ] || [ -z "$registry_fingerprint" ] || [ -z "$content_fingerprint" ]; then
    echo "brawler routed network: invalid routing identity output" >&2
    exit 1
fi

metrics_args=
if [ -n "$metrics_file" ]; then
    # This optional path is only an evidence snapshot destination. The canonical launcher keeps
    # the normal transport and worker argv unchanged when it is omitted.
    metrics_args="--metrics-file $metrics_file"
fi
if [ -n "$window_dir" ]; then
    mkdir -p "$window_dir"
fi
target/debug/brawler-supervisor $metrics_args \
    --network-protocol "$network_protocol" \
    --protocol-registry-fingerprint "$registry_fingerprint" \
    --content-fingerprint "$content_fingerprint" \
    --worker-executable "$project_dir/target/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --automatic-transition-driver \
    --mode "$game_mode" \
    --match-rules "$match_rules" \
    --bind "$bind_addr" &
supervisor_pid=$!

# Netcode retries its initial request, so no wall-clock readiness file or fixed sleep is required.
if [ "$headless" = 1 ]; then
    if [ -n "$window_dir" ]; then
        BRAWLER_DIAGNOSTICS_WINDOW_FILE="$window_dir/client-1.window" \
        BRAWLER_DIAGNOSTICS_ROLE=client \
        target/debug/brawler-client --client-id 1 --server "$bind_addr" --transport routed-udp --auto-connect \
            --headless --exit-after-roster 2 --exit-after-lobby-return --simulation-ticks "$simulation_ticks" \
            --move-axis 1,0 --aim-axis 0,1 &
    else
        target/debug/brawler-client --client-id 1 --server "$bind_addr" --transport routed-udp --auto-connect \
            --headless --exit-after-roster 2 --exit-after-lobby-return --simulation-ticks "$simulation_ticks" \
            --move-axis 1,0 --aim-axis 0,1 &
    fi
    client_one_pid=$!
    if [ -n "$window_dir" ]; then
        BRAWLER_DIAGNOSTICS_WINDOW_FILE="$window_dir/client-2.window" \
        BRAWLER_DIAGNOSTICS_ROLE=client \
        target/debug/brawler-client --client-id 2 --server "$bind_addr" --transport routed-udp --auto-connect \
            --headless --exit-after-roster 2 --exit-after-lobby-return --simulation-ticks "$simulation_ticks" \
            --move-axis -1,0 --aim-axis 0,-1 &
    else
        target/debug/brawler-client --client-id 2 --server "$bind_addr" --transport routed-udp --auto-connect \
            --headless --exit-after-roster 2 --exit-after-lobby-return --simulation-ticks "$simulation_ticks" \
            --move-axis -1,0 --aim-axis 0,-1 &
    fi
    client_two_pid=$!
    (
        sleep "$timeout_seconds"
        echo "brawler routed network: timed out after ${timeout_seconds}s" >&2
        kill -TERM "$$"
    ) &
    watchdog_pid=$!
else
    target/debug/brawler-client --client-id 1 --server "$bind_addr" --transport routed-udp --auto-connect &
    client_one_pid=$!
    target/debug/brawler-client --client-id 2 --server "$bind_addr" --transport routed-udp --auto-connect &
    client_two_pid=$!
fi

client_one_status=0
client_two_status=0
wait "$client_one_pid" || client_one_status=$?
client_one_pid=
wait "$client_two_pid" || client_two_status=$?
client_two_pid=
if [ -n "$watchdog_pid" ]; then
    kill "$watchdog_pid" 2>/dev/null || true
    wait "$watchdog_pid" 2>/dev/null || true
    watchdog_pid=
fi
if [ "$client_one_status" -ne 0 ] || [ "$client_two_status" -ne 0 ]; then
    echo "brawler routed network: client failure ($client_one_status, $client_two_status)" >&2
    exit 1
fi

kill -INT "$supervisor_pid"
wait "$supervisor_pid"
supervisor_pid=
echo "brawler routed network: two-client lobby-to-match-to-fresh-lobby transition passed"
