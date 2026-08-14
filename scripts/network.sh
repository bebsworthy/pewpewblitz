#!/usr/bin/env bash
set -euo pipefail

network_addr="${BRAWLER_NETWORK_ADDR:-127.0.0.1:5000}"
headless="${BRAWLER_NETWORK_HEADLESS:-0}"
combat_assert="${BRAWLER_NETWORK_ASSERT_COMBAT:-0}"
windowed_combat_demo="${BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO:-0}"
windowed_controller_demo="${BRAWLER_NETWORK_WINDOWED_CONTROLLER_DEMO:-0}"
combat_report_file="${BRAWLER_NETWORK_COMBAT_REPORT_FILE:-}"
network_run_id="${BRAWLER_NETWORK_RUN_ID:-network-script}"
network_timeout_seconds="${BRAWLER_NETWORK_TIMEOUT_SECONDS:-}"
startup_timeout_seconds=10
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
server_binary="$repo_root/target/debug/brawler-server"
client_binary="$repo_root/target/debug/brawler-client"

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
if [[ "$combat_assert" != "0" && "$combat_assert" != "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_ASSERT_COMBAT must be 0 or 1\n' >&2
    exit 2
fi
if [[ "$windowed_combat_demo" != "0" && "$windowed_combat_demo" != "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO must be 0 or 1\n' >&2
    exit 2
fi
if [[ "$windowed_controller_demo" != "0" && "$windowed_controller_demo" != "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_WINDOWED_CONTROLLER_DEMO must be 0 or 1\n' >&2
    exit 2
fi
if [[ "$combat_assert" == "1" && "$headless" != "1" ]]; then
    printf 'brawler network: combat assertion requires BRAWLER_NETWORK_HEADLESS=1\n' >&2
    exit 2
fi
if [[ "$windowed_combat_demo" == "1" && "$headless" == "1" ]]; then
    printf 'brawler network: windowed combat demo requires BRAWLER_NETWORK_HEADLESS=0\n' >&2
    exit 2
fi
if [[ "$windowed_controller_demo" == "1" && "$headless" == "1" ]]; then
    printf 'brawler network: windowed controller demo requires BRAWLER_NETWORK_HEADLESS=0\n' >&2
    exit 2
fi
if [[ "$windowed_combat_demo" == "1" && "$windowed_controller_demo" == "1" ]]; then
    printf 'brawler network: combat and controller demos cannot be combined\n' >&2
    exit 2
fi

server_pid=""
client_one_pid=""
client_two_pid=""
ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-server-ready.XXXXXX")"
movement_ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-movement-ready.XXXXXX")"
rm -f "$movement_ready_file"
combat_ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-combat-ready.XXXXXX")"
rm -f "$combat_ready_file"
combat_client_ready_dir="${BRAWLER_NETWORK_COMBAT_READY_DIR:-}"
combat_client_ready_dir_owned=0
if [[ -z "$combat_client_ready_dir" ]]; then
    combat_client_ready_dir="$(mktemp -d "${TMPDIR:-/tmp}/brawler-combat-clients.XXXXXX")"
    combat_client_ready_dir_owned=1
fi
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
    # Background Brawler processes ignore terminal SIGINT so the launcher owns Ctrl-C and can
    # terminate every child deterministically. Use SIGTERM for both normal and interrupted exits.
    local signal=TERM
    for pid in "$client_one_pid" "$client_two_pid" "$server_pid"; do
        stop_child "$pid" "$signal"
    done
    rm -f "$ready_file"
    rm -f "$movement_ready_file"
    rm -f "$combat_ready_file"
    if [[ "$combat_client_ready_dir_owned" -eq 1 ]]; then
        rmdir "$combat_client_ready_dir" 2>/dev/null || true
    fi
    exit "$exit_code"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

cargo build --locked --manifest-path "$repo_root/Cargo.toml" --no-default-features --features server --bin brawler-server
cargo build --locked --manifest-path "$repo_root/Cargo.toml" --no-default-features --features client --bin brawler-client

server_env=(env "BRAWLER_SERVER_READY_FILE=$ready_file")
if [[ "$headless" == "1" ]]; then
    if [[ "$combat_assert" == "1" ]]; then
        server_env+=(
            "BRAWLER_NETWORK_ASSERT_COMBAT=1"
            "BRAWLER_NETWORK_COMBAT_READY_FILE=$combat_ready_file"
            "BRAWLER_NETWORK_COMBAT_READY_DIR=$combat_client_ready_dir"
            "BRAWLER_NETWORK_RUN_ID=$network_run_id"
        )
        if [[ -n "$combat_report_file" ]]; then
            server_env+=("BRAWLER_NETWORK_COMBAT_REPORT_FILE=$combat_report_file")
        fi
    else
        server_env+=(
            "BRAWLER_NETWORK_ASSERT_MOVEMENT=1"
            "BRAWLER_NETWORK_MOVEMENT_READY_FILE=$movement_ready_file"
        )
    fi
fi

(trap '' INT; exec "${server_env[@]}" "$server_binary" --bind "$network_addr") &
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
    if [[ "$combat_assert" == "1" ]]; then
        client_args+=(--headless --exit-after-roster 2 --simulation-ticks 240 --fire)
    else
        client_args+=(--headless --exit-after-roster 2 --simulation-ticks 180)
    fi
fi

client_one_args=("${client_args[@]}" --client-id 1)
client_two_args=("${client_args[@]}" --client-id 2)
if [[ "$windowed_combat_demo" == "1" ]]; then
    client_one_args+=(--combat-demo)
fi
if [[ "$windowed_controller_demo" == "1" ]]; then
    client_one_args+=(--controller-demo)
fi
if [[ "$headless" == "1" ]]; then
    if [[ "$combat_assert" == "1" ]]; then
        client_one_args+=(--move-axis 0,0 --aim-dummy)
        client_two_args+=(--move-axis 0,0 --aim-dummy)
    else
        client_one_args+=(--move-axis 1,0 --aim-axis 0,1)
        client_two_args+=(--move-axis -1,0 --aim-axis 0,-1)
    fi
fi

client_one_env=(env)
client_two_env=(env)
if [[ "$combat_assert" == "1" ]]; then
    client_one_env+=("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE=$combat_client_ready_dir/client-1.ready")
    client_two_env+=("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE=$combat_client_ready_dir/client-2.ready")
fi

(trap '' INT; exec "${client_one_env[@]}" "$client_binary" "${client_one_args[@]}") &
client_one_pid=$!
(trap '' INT; exec "${client_two_env[@]}" "$client_binary" "${client_two_args[@]}") &
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
        if [[ "$combat_assert" == "1" ]]; then
            if [[ -s "$combat_ready_file" \
                && -s "$combat_client_ready_dir/client-1.ready" \
                && -s "$combat_client_ready_dir/client-2.ready" ]]; then
                exit 0
            fi
            printf 'brawler network: clients finished before combat assertion completed; waiting for server evidence\n' >&2
        elif [[ -s "$movement_ready_file" ]]; then
            exit 0
        else
            printf 'brawler network: clients finished before movement assertion completed; waiting for server evidence\n' >&2
        fi
    fi
    if [[ "$deadline_epoch" -gt 0 && "$(date +%s)" -ge "$deadline_epoch" ]]; then
        if [[ "$combat_assert" == "1" ]]; then
            printf 'brawler network: combat client evidence directory:\n' >&2
            ls -la "$combat_client_ready_dir" >&2 || true
            if [[ -n "$combat_report_file" && -f "$combat_report_file" ]]; then
                cat "$combat_report_file" >&2
            fi
        fi
        printf 'brawler network: timed out after %s seconds; server=%s client1=%s client2=%s\n' \
            "$network_timeout_seconds" "$server_done" "$client_one_done" "$client_two_done" >&2
        exit 124
    fi
    sleep 0.1
done
