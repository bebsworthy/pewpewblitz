#!/usr/bin/env bash
set -euo pipefail

network_addr="${BRAWLER_NETWORK_ADDR:-127.0.0.1:5000}"
game_mode="${BRAWLER_NETWORK_GAME_MODE:-wipeout}"
headless="${BRAWLER_NETWORK_HEADLESS:-0}"
windowed_combat_demo="${BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO:-0}"
windowed_controller_demo="${BRAWLER_NETWORK_WINDOWED_CONTROLLER_DEMO:-0}"
screenshot_dir="${BRAWLER_NETWORK_SCREENSHOT_DIR:-}"
screenshot_first="${BRAWLER_NETWORK_SCREENSHOT_FIRST:-30}"
network_run_id="${BRAWLER_NETWORK_RUN_ID:-network-script}"
diagnostics_dir="${BRAWLER_DIAGNOSTICS_DIR:-}"
client_count="${BRAWLER_NETWORK_CLIENT_COUNT:-2}"
server_features="${BRAWLER_NETWORK_SERVER_FEATURES:-server}"
# brawler-client package features; shared-crate features are spelled `brawler/<feature>`.
client_features="${BRAWLER_NETWORK_CLIENT_FEATURES:-automation}"
diagnostics_scenario_id="${BRAWLER_DIAGNOSTICS_SCENARIO_ID:-}"
network_timeout_seconds="${BRAWLER_NETWORK_TIMEOUT_SECONDS:-}"
simulation_ticks="${BRAWLER_NETWORK_SIMULATION_TICKS:-}"
startup_timeout_seconds=10
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
    target_dir="$repo_root/$target_dir"
fi
server_binary="$target_dir/debug/brawler-server"
client_binary="$target_dir/debug/brawler-client"

if [[ -z "$network_timeout_seconds" ]]; then
    if [[ "$headless" == "1" ]]; then
        network_timeout_seconds=30
    else
        network_timeout_seconds=0
    fi
fi
if ! [[ "$client_count" =~ ^[1-8]$ ]]; then
    printf 'brawler network: BRAWLER_NETWORK_CLIENT_COUNT must be 1-8\n' >&2
    exit 2
fi
if ! [[ "$network_timeout_seconds" =~ ^[0-9]+$ ]]; then
    printf 'brawler network: BRAWLER_NETWORK_TIMEOUT_SECONDS must be a non-negative integer\n' >&2
    exit 2
fi
if [[ -n "$simulation_ticks" ]] && ! [[ "$simulation_ticks" =~ ^[1-9][0-9]*$ ]]; then
    printf 'brawler network: BRAWLER_NETWORK_SIMULATION_TICKS must be a positive integer\n' >&2
    exit 2
fi
if [[ "$headless" != "0" && "$headless" != "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_HEADLESS must be 0 or 1\n' >&2
    exit 2
fi
if [[ "$game_mode" != "wipeout" && "$game_mode" != "hot-zone" && "$game_mode" != "heist" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_GAME_MODE must be wipeout, hot-zone, or heist\n' >&2
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
if [[ -n "$screenshot_dir" && "$headless" == "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_SCREENSHOT_DIR requires a windowed client\n' >&2
    exit 2
fi
if ! [[ "$screenshot_first" =~ ^[1-9][0-9]*$ ]]; then
    printf 'brawler network: BRAWLER_NETWORK_SCREENSHOT_FIRST must be a positive integer\n' >&2
    exit 2
fi

server_pid=""
# Every spawned client is tracked uniformly: exit status, cleanup, and report validation
# cover the whole 1-8 roster, not just the first two clients.
client_pids=()
client_done=()
ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-server-ready.XXXXXX")"
movement_ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-movement-ready.XXXXXX")"
rm -f "$movement_ready_file"
server_done=0

job_is_running() {
    jobs -pr | grep -qx "$1"
}

all_clients_done() {
    # Bash 3.2 treats expanding a declared-but-empty array as unbound under `set -u`, so
    # every expansion of the client roster is guarded by an explicit length check.
    if [[ "${#client_pids[@]}" -eq 0 ]]; then
        return 0
    fi
    for pid in "${client_pids[@]}"; do
        if [[ -n "$pid" ]] && job_is_running "$pid"; then
            return 1
        fi
    done
    return 0
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
    if [[ "${#client_pids[@]}" -gt 0 ]]; then
        for pid in "${client_pids[@]}"; do
            stop_child "$pid" "$signal"
        done
    fi
    stop_child "$server_pid" "$signal"
    rm -f "$ready_file"
    rm -f "$movement_ready_file"
    exit "$exit_code"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

validate_closeout_reports() {
    if [[ -z "$diagnostics_dir" ]]; then
        return 0
    fi
    # Reuse the binaries' own closeout-report reader so the terminal gate enforces the
    # full schema contract (every required field with typed values, bounded identities,
    # participant rows, the gameplay-aggregate consistency rules, one shared run identity
    # including source revision and the canonical participant/build assignment, clean
    # exits, and zero drops/rejections/errors) instead of a launcher-side subset that can
    # drift from the writer.
    "$server_binary" validate-closeout "$diagnostics_dir" "$client_count"
    printf 'brawler network: closeout reports validated in %s\n' "$diagnostics_dir"
    if command -v shasum >/dev/null 2>&1; then
        printf 'brawler network: terminal digest '
        cat "$diagnostics_dir"/*.closeout | shasum
    fi
}

wait_for_server_closeout() {
    if [[ -z "$diagnostics_dir" ]]; then
        return 0
    fi
    # The measurement server exits by itself once every enabled verification completes;
    # give the graceful stop path time to flush terminal evidence before validating.
    local waited=0
    while job_is_running "$server_pid" && [[ "$waited" -lt 150 ]]; do
        sleep 0.1
        waited=$((waited + 1))
    done
}

cargo build --locked --manifest-path "$repo_root/Cargo.toml" --target-dir "$target_dir" --no-default-features --features "$server_features" --bin brawler-server
cargo build --locked --manifest-path "$repo_root/Cargo.toml" --target-dir "$target_dir" -p brawler-client --features "$client_features" --bin brawler-client

server_env=(env "BRAWLER_SERVER_READY_FILE=$ready_file")
if [[ -n "$diagnostics_dir" ]]; then
    # Closeout-report identity for deterministic scenario reproduction. These are development
    # verification controls, not a v2 worker manifest or IPC contract.
    if [[ -z "$diagnostics_scenario_id" ]]; then
        diagnostics_scenario_id="network-${game_mode}-${network_run_id}"
    fi
    # Scenario declarations derived from the selected profile, not free-form metadata:
    # the seed names the deterministic reproduction (match ids, spawn selection, and map
    # recipes have no ambient randomness, so the declared seed is the scenario label all
    # endpoints must agree on); the scripted-action count is one per scripted input
    # channel the launcher applies to the two scripted headless clients (move axis and
    # aim source).
    diagnostics_seed="${BRAWLER_DIAGNOSTICS_SEED:-1}"
    scripted_actions=0
    if [[ "$headless" == "1" ]]; then
        scripted_actions=4
    fi
    source_revision="$(git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    if [[ -n "$(git -C "$repo_root" status --porcelain 2>/dev/null)" ]]; then
        source_dirty=1
    else
        source_dirty=0
    fi
    mkdir -p "$diagnostics_dir"
    identity_env=(
        "BRAWLER_DIAGNOSTICS_SCENARIO_ID=$diagnostics_scenario_id"
        "BRAWLER_NETWORK_RUN_ID=$network_run_id"
        "BRAWLER_SOURCE_REVISION=$source_revision"
        "BRAWLER_SOURCE_DIRTY=$source_dirty"
        "BRAWLER_DIAGNOSTICS_MODE=$game_mode"
        "BRAWLER_DIAGNOSTICS_SEED=$diagnostics_seed"
        "BRAWLER_DIAGNOSTICS_SCRIPTED_ACTIONS=$scripted_actions"
        "BRAWLER_SERVER_EXIT_AFTER_VERIFICATION=1"
    )
    server_env+=(
        "${identity_env[@]}"
        "BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE=$diagnostics_dir/server.closeout"
        "BRAWLER_DIAGNOSTICS_WINDOW_FILE=$diagnostics_dir/server.window"
        "BRAWLER_DIAGNOSTICS_ROLE=server"
    )
fi
if [[ "$headless" == "1" ]]; then
    server_env+=(
        "BRAWLER_NETWORK_ASSERT_MOVEMENT=1"
        "BRAWLER_NETWORK_MOVEMENT_READY_FILE=$movement_ready_file"
    )
fi

server_args=(--bind "$network_addr" --mode "$game_mode")
if [[ -n "${BRAWLER_NETWORK_MATCH_RULES:-}" ]]; then
    server_args+=(--match-rules "$BRAWLER_NETWORK_MATCH_RULES")
fi

(trap '' INT; exec "${server_env[@]}" "$server_binary" "${server_args[@]}") &
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

client_args=(--server "$network_addr" --auto-connect)
if [[ "$headless" == "1" ]]; then
    if [[ -n "$simulation_ticks" ]]; then
        client_args+=(--headless --exit-after-roster 2 --simulation-ticks "$simulation_ticks")
    else
        # Outlast the production countdown (180 ticks) plus travel so the movement
        # assertion observes real displacement instead of racing client shutdown.
        client_args+=(--headless --exit-after-roster 2 --simulation-ticks 600)
    fi
fi

client_one_args=("${client_args[@]}" --client-id 1)
client_two_args=("${client_args[@]}" --client-id 2)

if [[ -n "${BRAWLER_NETWORK_CHARACTER:-}" ]]; then
    client_one_args+=(--character "$BRAWLER_NETWORK_CHARACTER")
fi
if [[ -n "${BRAWLER_NETWORK_CHARACTER_CLIENT_TWO:-}" ]]; then
    client_two_args+=(--character "$BRAWLER_NETWORK_CHARACTER_CLIENT_TWO")
fi
if [[ "$windowed_combat_demo" == "1" ]]; then
    client_one_args+=(--combat-demo)
    client_two_args+=(--combat-demo)
fi
if [[ -n "$screenshot_dir" ]]; then
    client_one_args+=(--screenshot-dir "$screenshot_dir" --screenshot-first "$screenshot_first")
fi
if [[ "$windowed_controller_demo" == "1" ]]; then
    client_one_args+=(--controller-demo)
fi
if [[ "$headless" == "1" ]]; then
    client_one_args+=(--move-axis 1,0 --aim-axis 0,1)
    client_two_args+=(--move-axis -1,0 --aim-axis 0,-1)
fi

client_one_env=(env)
client_two_env=(env)
if [[ -n "$diagnostics_dir" ]]; then
    client_one_env+=(
        "${identity_env[@]}"
        "BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE=$diagnostics_dir/client-1.closeout"
        "BRAWLER_DIAGNOSTICS_WINDOW_FILE=$diagnostics_dir/client-1.window"
        "BRAWLER_DIAGNOSTICS_ROLE=client"
    )
    client_two_env+=(
        "${identity_env[@]}"
        "BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE=$diagnostics_dir/client-2.closeout"
        "BRAWLER_DIAGNOSTICS_WINDOW_FILE=$diagnostics_dir/client-2.window"
        "BRAWLER_DIAGNOSTICS_ROLE=client"
    )
fi

client_pids=()
for index in $(seq 1 "$client_count"); do
    case "$index" in
        1)
            envs=("${client_one_env[@]}")
            args=("${client_one_args[@]}")
            ;;
        2)
            envs=("${client_two_env[@]}")
            args=("${client_two_args[@]}")
            ;;
        *)
            args=(--server "$network_addr" --client-id "$index" --auto-connect)
            if [[ -n "${BRAWLER_NETWORK_CHARACTER:-}" ]]; then
                args+=(--character "$BRAWLER_NETWORK_CHARACTER")
            fi
            if [[ "$headless" == "1" ]]; then
                args+=(--headless --exit-after-roster "$client_count" --simulation-ticks 600)
            fi
            envs=(env)
            if [[ -n "$diagnostics_dir" ]]; then
                envs+=(
                    "${identity_env[@]}"
                    "BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE=$diagnostics_dir/client-$index.closeout"
                )
            fi
            ;;
    esac
    (trap '' INT; exec "${envs[@]}" "$client_binary" "${args[@]}") &
    client_pids+=("$!")
    client_done+=(0)
done

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
        if [[ "$server_exit_code" -eq 0 && -n "$diagnostics_dir" ]]; then
            # The measurement server exits by itself after completing every enabled
            # verification; let the clients finish their tick budgets before validating.
            continue
        fi
        if [[ "$server_exit_code" -eq 0 ]]; then
            exit 1
        fi
        exit "$server_exit_code"
    fi

    if [[ "${#client_pids[@]}" -gt 0 ]]; then
        for index in "${!client_pids[@]}"; do
            if [[ "${client_done[$index]}" -eq 0 ]] && ! job_is_running "${client_pids[$index]}"; then
                if wait "${client_pids[$index]}"; then
                    client_exit_code=0
                else
                    client_exit_code=$?
                fi
                client_done[$index]=1
                printf 'brawler network: client %s exited with status %s\n' \
                    "$((index + 1))" "$client_exit_code" >&2
                if [[ "$client_exit_code" -ne 0 ]]; then
                    exit "$client_exit_code"
                fi
            fi
        done
    fi

    if [[ "$headless" == "1" ]] && all_clients_done; then
        if [[ -s "$movement_ready_file" ]]; then
            wait_for_server_closeout
            validate_closeout_reports
            exit 0
        else
            printf 'brawler network: clients finished before movement assertion completed; waiting for server evidence\n' >&2
        fi
    fi
    if [[ "$deadline_epoch" -gt 0 && "$(date +%s)" -ge "$deadline_epoch" ]]; then
        client_status="server=$server_done"
        if [[ "${#client_pids[@]}" -gt 0 ]]; then
            for index in "${!client_pids[@]}"; do
                client_status="$client_status client$((index + 1))=${client_done[$index]}"
            done
        fi
        printf 'brawler network: timed out after %s seconds; %s\n' \
            "$network_timeout_seconds" "$client_status" >&2
        exit 124
    fi
    sleep 0.1
done
