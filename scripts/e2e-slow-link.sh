#!/usr/bin/env bash
set -euo pipefail

# BRL-jx6p: one headless client reaches the authenticated lobby welcome through a UDP proxy that
# delays every supervisor-to-client datagram, so each lobby round trip takes at least the delay.
# The run passes only if the client connects on its first routed socket: a retry on a fresh port
# would mean the pre-auth ingress allowance starved the handshake.

delay_ms=${1:-1500}
case "$delay_ms" in
    '' | *[!0-9]*)
        printf '%s\n' 'brawler slow-link e2e: expected the delay in milliseconds' >&2
        exit 2
        ;;
esac

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

free_port() {
    python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
}

bind_addr=${BRAWLER_ROUTED_BIND:-127.0.0.1:$(free_port)}
proxy_addr=${BRAWLER_SLOW_LINK_PROXY_BIND:-127.0.0.1:$(free_port)}
timeout_seconds=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-60}
run_dir=${BRAWLER_SLOW_LINK_RUN_DIR:-target/slow-link-e2e}
rm -rf "$run_dir"
mkdir -p "$run_dir"
supervisor_pid=
proxy_pid=
client_pid=
watchdog_pid=

cleanup() {
    trap - INT TERM EXIT
    # A background job of a non-interactive shell starts with SIGINT ignored, and the client and
    # proxy keep it ignored; only the supervisor installs its own SIGINT handler.
    for pid in "$watchdog_pid" "$client_pid" "$proxy_pid"; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    if [[ -n "$supervisor_pid" ]] && kill -0 "$supervisor_pid" 2>/dev/null; then
        kill -INT "$supervisor_pid" 2>/dev/null || true
    fi
    for pid in "$watchdog_pid" "$client_pid" "$supervisor_pid" "$proxy_pid"; do
        if [[ -n "$pid" ]]; then
            wait "$pid" 2>/dev/null || true
        fi
    done
}
trap cleanup INT TERM EXIT

cargo build --locked -p brawler-client --bin brawler-client --features automation
cargo build --locked --bin brawler-server --no-default-features --features server
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$(target/debug/brawler-server routing-identity)
network_protocol=$(printf '%s\n' "$identity" | awk -F= '$1 == "network_protocol" { print $2 }')
registry_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }')
content_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "content_fingerprint" { print $2 }')

printf 'brawler slow-link e2e: supervisor %s, proxy %s, server-to-client delay %s ms\n' \
    "$bind_addr" "$proxy_addr" "$delay_ms"
target/debug/brawler-supervisor \
    --network-protocol "$network_protocol" \
    --protocol-registry-fingerprint "$registry_fingerprint" \
    --content-fingerprint "$content_fingerprint" \
    --worker-executable "$project_dir/target/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --data-directory "$run_dir/server" \
    --bind "$bind_addr" >"$run_dir/supervisor.log" 2>&1 &
supervisor_pid=$!
python3 scripts/udp-delay-proxy.py \
    --listen "$proxy_addr" --upstream "$bind_addr" --delay-ms "$delay_ms" \
    >"$run_dir/proxy.log" 2>&1 &
proxy_pid=$!

(
    sleep "$timeout_seconds"
    printf 'brawler slow-link e2e: timed out after %ss\n' "$timeout_seconds" >&2
    kill -TERM "$$"
) &
watchdog_pid=$!

started=$(python3 -c 'import time; print(time.monotonic())')
client_status=0
target/debug/brawler-client \
    --client-id 3101 \
    --server "$proxy_addr" \
    --transport routed-udp \
    --auto-connect \
    --headless \
    --exit-after-lobby-welcome >"$run_dir/client.log" 2>&1 &
client_pid=$!
wait "$client_pid" || client_status=$?
client_pid=
elapsed=$(python3 -c "import time; print(f'{time.monotonic() - $started:.2f}')")
kill "$watchdog_pid" 2>/dev/null || true
wait "$watchdog_pid" 2>/dev/null || true
watchdog_pid=

attempts=$(grep -c 'routed UDP socket bound' "$run_dir/client.log" || true)
limited=$(grep -c 'reason=source-limited' "$run_dir/supervisor.log" || true)
printf 'brawler slow-link e2e: client status %s after %ss, %s socket(s), %s source-limited line(s)\n' \
    "$client_status" "$elapsed" "$attempts" "$limited"
if [[ "$client_status" -ne 0 ]]; then
    printf 'brawler slow-link e2e: client failed; logs in %s\n' "$run_dir" >&2
    exit "$client_status"
fi
if [[ "$attempts" -ne 1 ]]; then
    printf 'brawler slow-link e2e: client needed %s connect attempts; logs in %s\n' \
        "$attempts" "$run_dir" >&2
    exit 1
fi

kill -INT "$supervisor_pid"
wait "$supervisor_pid"
supervisor_pid=
printf 'brawler slow-link e2e: lobby welcome reached through a %s ms delay\n' "$delay_ms"
