#!/usr/bin/env bash
set -euo pipefail

network_addr="${BRAWLER_NETWORK_ADDR:-127.0.0.1:5000}"
headless="${BRAWLER_NETWORK_HEADLESS:-0}"
network_timeout_seconds="${BRAWLER_NETWORK_TIMEOUT_SECONDS:-}"
startup_timeout_seconds=10

if [[ -z "$network_timeout_seconds" ]]; then
    if [[ "$headless" == "1" ]]; then
        network_timeout_seconds=30
    else
        network_timeout_seconds=0
    fi
fi
if ! [[ "$network_timeout_seconds" =~ ^[0-9]+$ ]]; then
    printf 'brawler network: BRAWLER_NETWORK_TIMEOUT_SECONDS must be a non-negative integer\n' >&2
    exit 2
fi
if [[ "$headless" != "0" && "$headless" != "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_HEADLESS must be 0 or 1\n' >&2
    exit 2
fi

server_pid=""
client_one_pid=""
client_two_pid=""
ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-server-ready.XXXXXX")"
movement_ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-movement-ready.XXXXXX")"
rm -f "$movement_ready_file"
server_done=0
client_one_done=0
client_two_done=0

job_is_running() {
    jobs -pr | grep -qx "$1"
}

stop_child() {
    local pid="$1"
    local signal="$2"
    if [[ -z "$pid" ]]; then
        return
    fi
    if job_is_running "$pid"; then
        kill -"$signal" "$pid" 2>/dev/null || true
        for _ in $(seq 1 30); do
            if ! job_is_running "$pid"; then
                break
            fi
            sleep 0.1
        done
        if job_is_running "$pid"; then
            kill -TERM "$pid" 2>/dev/null || true
            sleep 0.2
        fi
        if job_is_running "$pid"; then
            kill -KILL "$pid" 2>/dev/null || true
        fi
    fi
    wait "$pid" 2>/dev/null || true
}

cleanup() {
    local exit_code=$?
    local signal=INT
    if [[ "$exit_code" -ne 0 && "$exit_code" -ne 130 && "$exit_code" -ne 143 ]]; then
        signal=TERM
    fi
    for pid in "$client_one_pid" "$client_two_pid" "$server_pid"; do
        stop_child "$pid" "$signal"
    done
    rm -f "$ready_file"
    rm -f "$movement_ready_file"
    exit "$exit_code"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

cargo build --locked --no-default-features --features server --bin brawler-server
cargo build --locked --no-default-features --features client --bin brawler-client

server_env=(env "BRAWLER_SERVER_READY_FILE=$ready_file")
if [[ "$headless" == "1" ]]; then
    server_env+=(
        "BRAWLER_NETWORK_ASSERT_MOVEMENT=1"
        "BRAWLER_NETWORK_MOVEMENT_READY_FILE=$movement_ready_file"
    )
fi

(trap - INT TERM; exec "${server_env[@]}" cargo run --locked --no-default-features --features server --bin brawler-server -- --bind "$network_addr") &
server_pid=$!

start_epoch=$(date +%s)
startup_deadline_epoch=$((start_epoch + startup_timeout_seconds))
while [[ ! -s "$ready_file" ]]; do
    if ! job_is_running "$server_pid"; then
        if wait "$server_pid"; then
            server_exit_code=0
        else
            server_exit_code=$?
        fi
        printf 'brawler network: server exited before readiness with status %s\n' "$server_exit_code" >&2
        exit 1
    fi
    if [[ "$(date +%s)" -ge "$startup_deadline_epoch" ]]; then
        printf 'brawler network: server did not become ready after %s seconds\n' "$startup_timeout_seconds" >&2
        exit 124
    fi
    sleep 0.1
done

client_args=(--server "$network_addr")
if [[ "$headless" == "1" ]]; then
    client_args+=(--headless --exit-after-roster 2 --simulation-ticks 180)
fi

client_one_args=("${client_args[@]}" --client-id 1)
client_two_args=("${client_args[@]}" --client-id 2)
if [[ "$headless" == "1" ]]; then
    client_one_args+=(--move-axis 1,0 --aim-axis 0,1)
    client_two_args+=(--move-axis -1,0 --aim-axis 0,-1)
fi

(trap - INT TERM; exec cargo run --locked --no-default-features --features client --bin brawler-client -- "${client_one_args[@]}") &
client_one_pid=$!
(trap - INT TERM; exec cargo run --locked --no-default-features --features client --bin brawler-client -- "${client_two_args[@]}") &
client_two_pid=$!

start_epoch=$(date +%s)
if [[ "$network_timeout_seconds" -gt 0 ]]; then
    deadline_epoch=$((start_epoch + network_timeout_seconds))
else
    deadline_epoch=0
fi

while :; do
    if [[ "$server_done" -eq 0 ]] && ! job_is_running "$server_pid"; then
        if wait "$server_pid"; then
            server_exit_code=0
        else
            server_exit_code=$?
        fi
        server_done=1
        printf 'brawler network: server exited with status %s; stopping clients\n' "$server_exit_code" >&2
        if [[ "$server_exit_code" -eq 0 ]]; then
            exit 1
        fi
        exit "$server_exit_code"
    fi

    if [[ "$client_one_done" -eq 0 ]] && ! job_is_running "$client_one_pid"; then
        if wait "$client_one_pid"; then
            client_one_exit_code=0
        else
            client_one_exit_code=$?
        fi
        client_one_done=1
        printf 'brawler network: client 1 exited with status %s\n' "$client_one_exit_code" >&2
        if [[ "$client_one_exit_code" -ne 0 ]]; then
            exit "$client_one_exit_code"
        fi
    fi

    if [[ "$client_two_done" -eq 0 ]] && ! job_is_running "$client_two_pid"; then
        if wait "$client_two_pid"; then
            client_two_exit_code=0
        else
            client_two_exit_code=$?
        fi
        client_two_done=1
        printf 'brawler network: client 2 exited with status %s\n' "$client_two_exit_code" >&2
        if [[ "$client_two_exit_code" -ne 0 ]]; then
            exit "$client_two_exit_code"
        fi
    fi

    if [[ "$headless" == "1" && "$client_one_done" -eq 1 && "$client_two_done" -eq 1 ]]; then
        if [[ -s "$movement_ready_file" ]]; then
            exit 0
        fi
        printf 'brawler network: clients finished before movement assertion completed; waiting for server evidence\n' >&2
    fi
    if [[ "$deadline_epoch" -gt 0 && "$(date +%s)" -ge "$deadline_epoch" ]]; then
        printf 'brawler network: timed out after %s seconds; server=%s client1=%s client2=%s\n' \
            "$network_timeout_seconds" "$server_done" "$client_one_done" "$client_two_done" >&2
        exit 124
    fi
    sleep 0.1
done
