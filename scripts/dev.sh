#!/usr/bin/env bash
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bind_addr=${BRAWLER_ROUTED_BIND:-127.0.0.1:5000}
mode=${1:-}

usage() {
    printf '%s\n' 'usage: scripts/dev.sh server | client | run <client-count>' >&2
}

case "$mode" in
    server | client) ;;
    run)
        client_count=${2:-}
        case "$client_count" in
            '' | *[!0-9]* | 0)
                printf '%s\n' 'brawler dev: client count must be a positive integer' >&2
                exit 2
                ;;
        esac
        if [[ "$client_count" -gt 16 ]]; then
            printf '%s\n' 'brawler dev: client count must be between 1 and 16' >&2
            exit 2
        fi
        ;;
    *)
        usage
        exit 2
        ;;
esac

cd "$project_dir"

build_server() {
    cargo build --locked --bin brawler-server --no-default-features --features server
    cargo build --locked -p brawler-routing --bin brawler-supervisor
}

build_client() {
    cargo build --locked --bin brawler-client --no-default-features --features client
}

supervisor_args() {
    local identity network_protocol registry_fingerprint content_fingerprint
    identity=$(target/debug/brawler-server routing-identity)
    network_protocol=$(awk -F= '$1 == "network_protocol" { print $2 }' <<<"$identity")
    registry_fingerprint=$(awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }' <<<"$identity")
    content_fingerprint=$(awk -F= '$1 == "content_fingerprint" { print $2 }' <<<"$identity")
    if [[ -z "$network_protocol" || -z "$registry_fingerprint" || -z "$content_fingerprint" ]]; then
        printf '%s\n' 'brawler dev: server returned an incomplete routing identity' >&2
        return 1
    fi
    SUPERVISOR_ARGS=(
        --network-protocol "$network_protocol"
        --protocol-registry-fingerprint "$registry_fingerprint"
        --content-fingerprint "$content_fingerprint"
        --worker-executable "$project_dir/target/debug/brawler-server"
        --game-types "$project_dir/config/server/game-types.ron"
        --bind "$bind_addr"
    )
}

case "$mode" in
    server)
        build_server
        supervisor_args
        exec target/debug/brawler-supervisor "${SUPERVISOR_ARGS[@]}"
        ;;
    client)
        build_client
        exec target/debug/brawler-client --server "$bind_addr" --transport routed-udp
        ;;
esac

build_server
build_client
supervisor_args

supervisor_pid=
client_pids=()

cleanup() {
    local status=$?
    trap - EXIT INT TERM
    for pid in "${client_pids[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    if [[ -n "$supervisor_pid" ]] && kill -0 "$supervisor_pid" 2>/dev/null; then
        kill -INT "$supervisor_pid" 2>/dev/null || true
    fi
    for pid in "${client_pids[@]}"; do
        wait "$pid" 2>/dev/null || true
    done
    if [[ -n "$supervisor_pid" ]]; then
        wait "$supervisor_pid" 2>/dev/null || true
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

target/debug/brawler-supervisor "${SUPERVISOR_ARGS[@]}" &
supervisor_pid=$!

for ((index = 1; index <= client_count; index++)); do
    target/debug/brawler-client --server "$bind_addr" --transport routed-udp &
    client_pids+=("$!")
done

printf 'brawler dev: running %s interactive client(s) against %s; press Ctrl-C to stop\n' \
    "$client_count" "$bind_addr"

job_is_running() {
    jobs -pr | grep -qx "$1"
}

while job_is_running "$supervisor_pid"; do
    live_clients=0
    for pid in "${client_pids[@]}"; do
        if job_is_running "$pid"; then
            live_clients=$((live_clients + 1))
        fi
    done
    if [[ "$live_clients" -eq 0 ]]; then
        exit 0
    fi
    sleep 0.2
done

wait "$supervisor_pid"
