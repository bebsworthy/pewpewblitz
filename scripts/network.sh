#!/usr/bin/env bash
set -euo pipefail

network_addr="${BRAWLER_NETWORK_ADDR:-127.0.0.1:5000}"
headless="${BRAWLER_NETWORK_HEADLESS:-0}"

server_pid=""
client_one_pid=""
client_two_pid=""
server_done=0
client_one_done=0
client_two_done=0

job_is_running() {
    jobs -pr | grep -qx "$1"
}

cleanup() {
    local status=$?
    for pid in "$client_one_pid" "$client_two_pid" "$server_pid"; do
        if [[ -n "$pid" ]] && job_is_running "$pid"; then
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    for pid in "$client_one_pid" "$client_two_pid" "$server_pid"; do
        if [[ -n "$pid" ]]; then
            wait "$pid" 2>/dev/null || true
        fi
    done
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

cargo build --locked --no-default-features --features server --bin brawler-server
cargo build --locked --no-default-features --features client --bin brawler-client

(exec cargo run --locked --no-default-features --features server --bin brawler-server -- --bind "$network_addr") &
server_pid=$!

client_args=(--server "$network_addr")
if [[ "$headless" == "1" ]]; then
    client_args+=(--headless --exit-after-roster 2)
fi

(exec cargo run --locked --no-default-features --features client --bin brawler-client -- "${client_args[@]}" --client-id 1) &
client_one_pid=$!
(exec cargo run --locked --no-default-features --features client --bin brawler-client -- "${client_args[@]}" --client-id 2) &
client_two_pid=$!

while :; do
    if [[ "$server_done" -eq 0 ]] && ! job_is_running "$server_pid"; then
        if wait "$server_pid"; then
            server_status=0
        else
            server_status=$?
        fi
        server_done=1
        printf 'brawler network: server exited with status %s; stopping clients\n' "$server_status" >&2
        if [[ "$server_status" -eq 0 ]]; then
            exit 1
        fi
        exit "$server_status"
    fi

    if [[ "$client_one_done" -eq 0 ]] && ! job_is_running "$client_one_pid"; then
        if wait "$client_one_pid"; then
            client_one_status=0
        else
            client_one_status=$?
        fi
        client_one_done=1
        if [[ "$headless" == "1" && "$client_one_status" -ne 0 ]]; then
            printf 'brawler network: client 1 failed with status %s\n' "$client_one_status" >&2
            exit "$client_one_status"
        fi
        if [[ "$headless" != "1" ]]; then
            exit "$client_one_status"
        fi
    fi

    if [[ "$client_two_done" -eq 0 ]] && ! job_is_running "$client_two_pid"; then
        if wait "$client_two_pid"; then
            client_two_status=0
        else
            client_two_status=$?
        fi
        client_two_done=1
        if [[ "$headless" == "1" && "$client_two_status" -ne 0 ]]; then
            printf 'brawler network: client 2 failed with status %s\n' "$client_two_status" >&2
            exit "$client_two_status"
        fi
        if [[ "$headless" != "1" ]]; then
            exit "$client_two_status"
        fi
    fi

    if [[ "$headless" == "1" && "$client_one_done" -eq 1 && "$client_two_done" -eq 1 ]]; then
        exit 0
    fi
    sleep 0.1
done
