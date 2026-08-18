#!/usr/bin/env bash
set -euo pipefail

network_addr="${BRAWLER_NETWORK_ADDR:-127.0.0.1:5000}"
game_mode="${BRAWLER_NETWORK_GAME_MODE:-wipeout}"
headless="${BRAWLER_NETWORK_HEADLESS:-0}"
combat_assert="${BRAWLER_NETWORK_ASSERT_COMBAT:-0}"
terrain_assert="${BRAWLER_NETWORK_ASSERT_TERRAIN:-0}"
windowed_combat_demo="${BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO:-0}"
windowed_controller_demo="${BRAWLER_NETWORK_WINDOWED_CONTROLLER_DEMO:-0}"
combat_report_file="${BRAWLER_NETWORK_COMBAT_REPORT_FILE:-}"
combat_test_preset="${BRAWLER_NETWORK_COMBAT_TEST_PRESET:-}"
network_run_id="${BRAWLER_NETWORK_RUN_ID:-network-script}"
diagnostics_dir="${BRAWLER_DIAGNOSTICS_DIR:-}"
client_count="${BRAWLER_NETWORK_CLIENT_COUNT:-2}"
server_features="${BRAWLER_NETWORK_SERVER_FEATURES:-server}"
diagnostics_scenario_id="${BRAWLER_DIAGNOSTICS_SCENARIO_ID:-}"
network_timeout_seconds="${BRAWLER_NETWORK_TIMEOUT_SECONDS:-}"
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
        if [[ "$combat_assert" == "1" ]]; then
            network_timeout_seconds=60
        elif [[ "$terrain_assert" == "1" ]]; then
            network_timeout_seconds=45
        else
            network_timeout_seconds=30
        fi
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
if [[ "$headless" != "0" && "$headless" != "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_HEADLESS must be 0 or 1\n' >&2
    exit 2
fi
if [[ "$terrain_assert" != "0" && "$terrain_assert" != "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_ASSERT_TERRAIN must be 0 or 1\n' >&2
    exit 2
fi
if [[ "$terrain_assert" == "1" && "$headless" != "1" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_ASSERT_TERRAIN requires BRAWLER_NETWORK_HEADLESS=1\n' >&2
    exit 2
fi
if [[ "$terrain_assert" == "1" \
    && ( "${BRAWLER_NETWORK_WEAPON_PRESET:-3}" != "3" \
        || "${BRAWLER_NETWORK_WEAPON_PRESET_CLIENT_TWO:-${BRAWLER_NETWORK_WEAPON_PRESET:-3}}" != "3" ) ]]; then
    printf 'brawler network: BRAWLER_NETWORK_ASSERT_TERRAIN requires the Arc Launcher (preset 3) terrain world effect\n' >&2
    exit 2
fi
if [[ "$game_mode" != "wipeout" && "$game_mode" != "hot-zone" ]]; then
    printf 'brawler network: BRAWLER_NETWORK_GAME_MODE must be wipeout or hot-zone\n' >&2
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
# Every spawned client is tracked uniformly: exit status, cleanup, and report validation
# cover the whole 1-8 roster, not just the first two clients.
client_pids=()
client_done=()
ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-server-ready.XXXXXX")"
movement_ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-movement-ready.XXXXXX")"
rm -f "$movement_ready_file"
terrain_ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-terrain-ready.XXXXXX")"
rm -f "$terrain_ready_file"
terrain_report_file="${BRAWLER_NETWORK_TERRAIN_REPORT_FILE:-}"
combat_ready_file="$(mktemp "${TMPDIR:-/tmp}/brawler-combat-ready.XXXXXX")"
rm -f "$combat_ready_file"
combat_client_ready_dir="${BRAWLER_NETWORK_COMBAT_READY_DIR:-}"
combat_client_ready_dir_owned=0
if [[ -z "$combat_client_ready_dir" ]]; then
    combat_client_ready_dir="$(mktemp -d "${TMPDIR:-/tmp}/brawler-combat-clients.XXXXXX")"
    combat_client_ready_dir_owned=1
fi
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
    rm -f "$terrain_ready_file"
    rm -f "$combat_ready_file"
    if [[ "$combat_client_ready_dir_owned" -eq 1 ]]; then
        rmdir "$combat_client_ready_dir" 2>/dev/null || true
    fi
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
    # exits, zero drops/rejections/errors, and cross-endpoint digest agreement) instead
    # of a launcher-side subset that can drift from the writer. Combat-assert runs must
    # carry nonzero checkpoint digests; movement/terrain/match profiles record no combat
    # checkpoints, so their digests must be zero.
    "$server_binary" validate-closeout "$diagnostics_dir" "$client_count" "$combat_assert"
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
cargo build --locked --manifest-path "$repo_root/Cargo.toml" --target-dir "$target_dir" --no-default-features --features client --bin brawler-client

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
    # channel the launcher applies to the two scripted headless clients (move axis, aim
    # source, and fire where the profile fires); combat-assert runs declare the six
    # named checkpoints the assertion watches (four active_* weapon effects plus defeat
    # and reset — observed evidence overwrites the declared count at closeout).
    diagnostics_seed="${BRAWLER_DIAGNOSTICS_SEED:-1}"
    scripted_actions=0
    declared_checkpoints=0
    if [[ "$headless" == "1" ]]; then
        if [[ "$combat_assert" == "1" ]]; then
            scripted_actions=6
            declared_checkpoints=6
        elif [[ "$terrain_assert" == "1" ]]; then
            scripted_actions=6
        else
            scripted_actions=4
        fi
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
        "BRAWLER_DIAGNOSTICS_CHECKPOINTS=$declared_checkpoints"
        "BRAWLER_SERVER_EXIT_AFTER_VERIFICATION=1"
    )
    server_env+=(
        "${identity_env[@]}"
        "BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE=$diagnostics_dir/server.closeout"
    )
fi
if [[ "$headless" == "1" ]]; then
    if [[ "$combat_assert" == "1" ]]; then
        server_env+=(
            "BRAWLER_NETWORK_ASSERT_COMBAT=1"
            "BRAWLER_NETWORK_COMBAT_READY_FILE=$combat_ready_file"
            "BRAWLER_NETWORK_COMBAT_READY_DIR=$combat_client_ready_dir"
            "BRAWLER_NETWORK_RUN_ID=$network_run_id"
        )
        if [[ -n "$combat_test_preset" ]]; then
            server_env+=("BRAWLER_NETWORK_COMBAT_TEST_PRESET=$combat_test_preset")
        fi
        if [[ -n "$combat_report_file" ]]; then
            server_env+=("BRAWLER_NETWORK_COMBAT_REPORT_FILE=$combat_report_file")
        fi
    else
        if [[ "$terrain_assert" != "1" ]]; then
            server_env+=(
                "BRAWLER_NETWORK_ASSERT_MOVEMENT=1"
                "BRAWLER_NETWORK_MOVEMENT_READY_FILE=$movement_ready_file"
            )
        fi
    fi
    if [[ "$terrain_assert" == "1" ]]; then
        server_env+=(
            "BRAWLER_NETWORK_ASSERT_TERRAIN=1"
            "BRAWLER_NETWORK_TERRAIN_READY_FILE=$terrain_ready_file"
            "BRAWLER_NETWORK_TERRAIN_TEST_DUMMY=1"
        )
        if [[ -n "${BRAWLER_NETWORK_TERRAIN_TARGET_REVISION:-}" ]]; then
            server_env+=("BRAWLER_NETWORK_TERRAIN_TARGET_REVISION=$BRAWLER_NETWORK_TERRAIN_TARGET_REVISION")
        fi
        if [[ -n "$terrain_report_file" ]]; then
            server_env+=("BRAWLER_NETWORK_TERRAIN_REPORT_FILE=$terrain_report_file")
        fi
    fi
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

client_args=(--server "$network_addr")
if [[ "$headless" == "1" ]]; then
    if [[ "$combat_assert" == "1" ]]; then
        client_args+=(--headless --exit-after-roster 2 --simulation-ticks 1200 --fire)
    else
        # Outlast the production countdown (180 ticks) plus travel so the movement
        # assertion observes real displacement instead of racing client shutdown.
        # Terrain profiles keep walking longer so Arc Launcher lobbed deliveries
        # crater the central destructible block during the approach.
        if [[ "$terrain_assert" == "1" ]]; then
            client_args+=(--headless --exit-after-roster 2 --simulation-ticks 900)
        else
            client_args+=(--headless --exit-after-roster 2 --simulation-ticks 600)
        fi
    fi
fi

client_one_args=("${client_args[@]}" --client-id 1)
client_two_args=("${client_args[@]}" --client-id 2)

if [[ -n "${BRAWLER_NETWORK_WEAPON_PRESET:-}" ]]; then
    client_one_args+=(--build-preset "$BRAWLER_NETWORK_WEAPON_PRESET")
fi
if [[ -n "${BRAWLER_NETWORK_WEAPON_PRESET_CLIENT_TWO:-}" ]]; then
    client_two_args+=(--build-preset "$BRAWLER_NETWORK_WEAPON_PRESET_CLIENT_TWO")
fi
if [[ "$windowed_combat_demo" == "1" ]]; then
    client_one_args+=(--combat-demo)
fi
if [[ "$windowed_controller_demo" == "1" ]]; then
    client_one_args+=(--controller-demo)
fi
if [[ "$headless" == "1" ]]; then
    if [[ "$combat_assert" == "1" ]]; then
        # Walk the two clients toward the neutral dummy so every authored M05 delivery family
        # reaches its acceptance range (scatter and melee are intentionally short-range).
        if [[ "${BRAWLER_NETWORK_WEAPON_PRESET:-}" == "2" ]]; then
            client_one_args+=(--move-axis 0,0 --aim-dummy)
            client_two_args+=(--move-axis 0,0 --aim-dummy)
        elif [[ "${BRAWLER_NETWORK_WEAPON_PRESET:-}" == "3" || "${BRAWLER_NETWORK_WEAPON_PRESET:-}" == "4" ]]; then
            client_one_args+=(--move-axis 1,0 --aim-dummy)
            client_two_args+=(--move-axis 0,0 --aim-dummy)
        else
            client_one_args+=(--move-axis 1,0 --aim-dummy)
            client_two_args+=(--move-axis -1,0 --aim-dummy)
        fi
    else
        if [[ "$terrain_assert" == "1" ]]; then
            # Both Arc Launcher clients walk toward the terrain-profile practice target and
            # lob at it; the aimed landing sits just south of the central destructible block,
            # so every delivered brush erases real cells regardless of spawn lane or phase.
            client_one_args+=(--move-axis 1,0 --aim-dummy --fire)
            client_two_args+=(--move-axis -1,0 --aim-dummy --fire)
        else
            client_one_args+=(--move-axis 1,0 --aim-axis 0,1)
            client_two_args+=(--move-axis -1,0 --aim-axis 0,-1)
        fi
    fi
fi

client_one_env=(env)
client_two_env=(env)
if [[ "$combat_assert" == "1" ]]; then
    client_one_env+=("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE=$combat_client_ready_dir/client-1.ready")
    client_two_env+=("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE=$combat_client_ready_dir/client-2.ready")
fi
if [[ -n "$diagnostics_dir" ]]; then
    client_one_env+=(
        "${identity_env[@]}"
        "BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE=$diagnostics_dir/client-1.closeout"
    )
    client_two_env+=(
        "${identity_env[@]}"
        "BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE=$diagnostics_dir/client-2.closeout"
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
            args=(--server "$network_addr" --client-id "$index")
            if [[ -n "${BRAWLER_NETWORK_WEAPON_PRESET:-}" ]]; then
                args+=(--build-preset "$BRAWLER_NETWORK_WEAPON_PRESET")
            fi
            if [[ "$headless" == "1" && "$combat_assert" != "1" && "$terrain_assert" != "1" ]]; then
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
        if [[ "$combat_assert" == "1" ]]; then
            if [[ -s "$combat_ready_file" \
                && -s "$combat_client_ready_dir/client-1.ready" \
                && -s "$combat_client_ready_dir/client-2.ready" ]]; then
                wait_for_server_closeout
                validate_closeout_reports
                exit 0
            fi
            printf 'brawler network: clients finished before combat assertion completed; waiting for server evidence\n' >&2
        elif [[ "$terrain_assert" == "1" ]]; then
            if [[ -s "$terrain_ready_file" ]]; then
                wait_for_server_closeout
                validate_closeout_reports
                exit 0
            fi
            printf 'brawler network: clients finished before terrain assertion completed; waiting for server evidence\n' >&2
        elif [[ -s "$movement_ready_file" ]]; then
            wait_for_server_closeout
            validate_closeout_reports
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
        if [[ "$terrain_assert" == "1" && -n "$terrain_report_file" && -f "$terrain_report_file" ]]; then
            cat "$terrain_report_file" >&2
        fi
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
