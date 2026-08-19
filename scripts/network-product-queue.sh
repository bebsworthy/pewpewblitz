#!/bin/sh
set -eu

# M04 production queue smoke: one real routed lobby client observes the initial full snapshot,
# joins with a bounded build, observes the admission revision, cancels, observes the removal
# revision, and exits. The production lobby composition installs no allocation driver.

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bind_addr=${BRAWLER_ROUTED_BIND:-127.0.0.1:5000}
timeout_seconds=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-30}
build_preset=${BRAWLER_QUEUE_BUILD_PRESET:-1}
supervisor_pid=
client_pid=
watchdog_pid=

cleanup() {
    trap - INT TERM EXIT
    for pid in "$watchdog_pid" "$client_pid" "$supervisor_pid"; do
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            kill -INT "$pid" 2>/dev/null || true
        fi
    done
    for pid in "$watchdog_pid" "$client_pid" "$supervisor_pid"; do
        if [ -n "$pid" ]; then
            wait "$pid" 2>/dev/null || true
        fi
    done
}

abort() {
    cleanup
    exit 124
}

trap abort INT TERM
trap cleanup EXIT

cd "$project_dir"
cargo build --locked --bin brawler-client --no-default-features --features client
cargo build --locked --bin brawler-server --no-default-features --features server
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$(target/debug/brawler-server routing-identity)
network_protocol=$(printf '%s\n' "$identity" | awk -F= '$1 == "network_protocol" { print $2 }')
registry_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }')
content_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "content_fingerprint" { print $2 }')

BRAWLER_QUEUE_EVIDENCE=1 target/debug/brawler-supervisor \
    --network-protocol "$network_protocol" \
    --protocol-registry-fingerprint "$registry_fingerprint" \
    --content-fingerprint "$content_fingerprint" \
    --worker-executable "$project_dir/target/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --bind "$bind_addr" &
supervisor_pid=$!

target/debug/brawler-client \
    --client-id 4001 \
    --server "$bind_addr" \
    --transport routed-udp \
    --auto-connect \
    --headless \
    --product-queue-smoke \
    --build-preset "$build_preset" &
client_pid=$!

parent_pid=$$
(
    sleep "$timeout_seconds"
    echo "brawler product queue: timed out after ${timeout_seconds}s" >&2
    # Some POSIX shells defer a trapped signal while blocked in `wait`. Stop the
    # waited-on children first so the parent can run its TERM trap immediately.
    kill -TERM "$client_pid" 2>/dev/null || true
    kill -TERM "$supervisor_pid" 2>/dev/null || true
    kill -TERM "$parent_pid"
) &
watchdog_pid=$!

client_status=0
wait "$client_pid" || client_status=$?
client_pid=
kill "$watchdog_pid" 2>/dev/null || true
wait "$watchdog_pid" 2>/dev/null || true
watchdog_pid=
if [ "$client_status" -ne 0 ]; then
    echo "brawler product queue: client failed with status $client_status" >&2
    exit "$client_status"
fi

kill -INT "$supervisor_pid"
wait "$supervisor_pid"
supervisor_pid=
echo "brawler product queue: admission, fresh snapshot, cancellation, and cleanup passed without worker allocation"
