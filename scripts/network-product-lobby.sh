#!/bin/sh
set -eu

# M03 product-boundary smoke: a production lobby remains idle while two headless clients reach
# the authenticated welcome boundary concurrently. No automatic allocation driver is installed.

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bind_addr=${BRAWLER_ROUTED_BIND:-127.0.0.1:5000}
timeout_seconds=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-30}
headless=${BRAWLER_NETWORK_HEADLESS:-1}
case "$headless" in
    0 | 1) ;;
    *)
        echo "brawler product lobby: BRAWLER_NETWORK_HEADLESS must be 0 or 1" >&2
        exit 2
        ;;
esac
supervisor_pid=
client_pid=
client_two_pid=
watchdog_pid=

cleanup() {
    trap - INT TERM EXIT
    for pid in "$watchdog_pid" "$client_pid" "$client_two_pid" "$supervisor_pid"; do
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            kill -INT "$pid" 2>/dev/null || true
        fi
    done
    for pid in "$watchdog_pid" "$client_pid" "$client_two_pid" "$supervisor_pid"; do
        if [ -n "$pid" ]; then
            wait "$pid" 2>/dev/null || true
        fi
    done
}
trap cleanup INT TERM EXIT

cd "$project_dir"
cargo build --locked --bin brawler-client --no-default-features --features client
cargo build --locked --bin brawler-server --no-default-features --features server
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$(target/debug/brawler-server routing-identity)
network_protocol=$(printf '%s\n' "$identity" | awk -F= '$1 == "network_protocol" { print $2 }')
registry_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }')
content_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "content_fingerprint" { print $2 }')

target/debug/brawler-supervisor \
    --network-protocol "$network_protocol" \
    --protocol-registry-fingerprint "$registry_fingerprint" \
    --content-fingerprint "$content_fingerprint" \
    --worker-executable "$project_dir/target/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --bind "$bind_addr" &
supervisor_pid=$!

if [ "$headless" = 1 ]; then
    target/debug/brawler-client \
        --client-id 3001 \
        --server "$bind_addr" \
        --transport routed-udp \
        --auto-connect \
        --headless \
        --exit-after-lobby-welcome &
    client_pid=$!
    target/debug/brawler-client \
        --client-id 3002 \
        --server "$bind_addr" \
        --transport routed-udp \
        --auto-connect \
        --headless \
        --exit-after-lobby-welcome &
    client_two_pid=$!
else
    target/debug/brawler-client --server "$bind_addr" --transport routed-udp &
    client_pid=$!
fi

if [ "$headless" = 1 ]; then
    (
        sleep "$timeout_seconds"
        echo "brawler product lobby: timed out after ${timeout_seconds}s" >&2
        kill -TERM "$$"
    ) &
    watchdog_pid=$!
fi

client_status=0
wait "$client_pid" || client_status=$?
client_pid=
if [ -n "$client_two_pid" ]; then
    wait "$client_two_pid" || client_status=$?
    client_two_pid=
fi
kill "$watchdog_pid" 2>/dev/null || true
wait "$watchdog_pid" 2>/dev/null || true
watchdog_pid=
if [ "$client_status" -ne 0 ]; then
    echo "brawler product lobby: client failed with status $client_status" >&2
    exit "$client_status"
fi

kill -INT "$supervisor_pid"
wait "$supervisor_pid"
supervisor_pid=
if [ "$headless" = 1 ]; then
    echo "brawler product lobby: two authenticated welcomes passed without automatic allocation"
else
    echo "brawler product lobby: windowed session closed cleanly"
fi
