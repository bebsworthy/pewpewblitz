#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
    target_dir="$repo_root/$target_dir"
fi
network_addr="${BRAWLER_NETWORK_ADDR:-127.0.0.1:5200}"
custom_build_client="${BRAWLER_NETWORK_CUSTOM_BUILD_CLIENT:-0}"
match_rules="${BRAWLER_NETWORK_MATCH_RULES:-verification}"
game_mode="${BRAWLER_NETWORK_GAME_MODE:-wipeout}"
simulation_ticks="${BRAWLER_NETWORK_SIMULATION_TICKS:-6000}"
match_timeout_seconds="${BRAWLER_NETWORK_MATCH_TIMEOUT_SECONDS:-90}"
if [[ ! "$custom_build_client" =~ ^[0-4]$ ]]; then
    printf 'brawler match: BRAWLER_NETWORK_CUSTOM_BUILD_CLIENT must be between 0 and 4\n' >&2
    exit 2
fi
if [[ "$match_rules" != "verification" && "$match_rules" != "production" ]]; then
    printf 'brawler match: BRAWLER_NETWORK_MATCH_RULES must be verification or production\n' >&2
    exit 2
fi
if [[ "$game_mode" != "wipeout" && "$game_mode" != "hot-zone" && "$game_mode" != "heist" ]]; then
    printf 'brawler match: BRAWLER_NETWORK_GAME_MODE must be wipeout, hot-zone, or heist\n' >&2
    exit 2
fi
if [[ ! "$simulation_ticks" =~ ^[1-9][0-9]*$ || ! "$match_timeout_seconds" =~ ^[1-9][0-9]*$ ]]; then
    printf 'brawler match: simulation ticks and timeout must be positive integers\n' >&2
    exit 2
fi
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/brawler-match.XXXXXX")"
ready_file="$work_dir/server.ready"
report_file="${BRAWLER_NETWORK_MATCH_REPORT_FILE:-$work_dir/match.report}"
server_pid=""
client_pids=()

job_is_running() {
    jobs -pr | grep -qx "$1"
}

cleanup() {
    local exit_code=$?
    for pid in "${client_pids[@]-}" "$server_pid"; do
        if [[ -n "$pid" ]] && job_is_running "$pid"; then
            kill -TERM "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    rm -f "$ready_file"
    if [[ "$report_file" == "$work_dir/"* ]]; then
        rm -f "$report_file"
    fi
    rmdir "$work_dir" 2>/dev/null || true
    exit "$exit_code"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

cargo build --locked --manifest-path "$repo_root/Cargo.toml" --target-dir "$target_dir" \
    --no-default-features --features server --bin brawler-server
cargo build --locked --manifest-path "$repo_root/Cargo.toml" --target-dir "$target_dir" \
    --no-default-features --features client --bin brawler-client

(trap '' INT; exec env \
    BRAWLER_SERVER_READY_FILE="$ready_file" \
    BRAWLER_NETWORK_ASSERT_MATCH=1 \
    BRAWLER_NETWORK_MATCH_REPORT_FILE="$report_file" \
    "$target_dir/debug/brawler-server" --bind "$network_addr" --mode "$game_mode" --match-rules "$match_rules") &
server_pid=$!

deadline=$((SECONDS + 10))
while [[ ! -s "$ready_file" ]]; do
    if ! job_is_running "$server_pid"; then
        printf 'brawler match: server exited before readiness\n' >&2
        exit 1
    fi
    if ((SECONDS >= deadline)); then
        printf 'brawler match: server readiness timed out\n' >&2
        exit 124
    fi
    sleep 0.1
done

for client_id in 1 2 3 4; do
    move_axis="1,0"
    if ((client_id % 2 == 0)); then
        move_axis="-1,0"
    fi
    build_preset="$client_id"
    if ((custom_build_client == client_id)); then
        build_preset="5"
    fi
    client_args=(
        --server "$network_addr" --client-id "$client_id" --headless
        --exit-after-roster 4 --simulation-ticks "$simulation_ticks" --build-preset "$build_preset"
        --move-axis "$move_axis" --aim-dummy --fire --ultimate
    )
    (trap '' INT; exec "$target_dir/debug/brawler-client" "${client_args[@]}") &
    client_pids+=("$!")
done

deadline=$((SECONDS + match_timeout_seconds))
while job_is_running "$server_pid"; do
    if ((SECONDS >= deadline)); then
        printf 'brawler match: %s 2v2 match timed out\n' "$game_mode" >&2
        exit 124
    fi
    sleep 0.1
done
if ! wait "$server_pid"; then
    printf 'brawler match: server verification failed\n' >&2
    exit 1
fi
server_pid=""

for pid in "${client_pids[@]}"; do
    if job_is_running "$pid"; then
        kill -TERM "$pid" 2>/dev/null || true
    fi
    wait "$pid" 2>/dev/null || true
done
client_pids=()

for required in initial_match_id restarted_match_id participant_count mode_definition_id \
    summary_participant_count map_instance_id map_recipe_fingerprint content_fingerprint rules_revision \
    final_score_team_1 final_score_team_2 result active_duration_ticks defeats respawns participant_active_ticks_team_1 \
    participant_active_ticks_team_2 records dropped_records summary_count \
    weapon_aggregate_count weapon_preset_ids build_preset_ids custom_builds build_fingerprints \
    build_total_points ultimate_ids passive_ids first_full_charge_ticks first_full_charge_active_ticks ability_uses_by_owner \
    charge_dealt_by_owner charge_received_by_owner passive_triggers accepted_attacks \
    attacks_with_hostile_contact build_selections build_dropped_records ability_attempts \
    ability_accepts dash_uses sentry_uses \
    sentry_shots ability_dropped_records; do
    if ! grep -q "^${required}=" "$report_file"; then
        printf 'brawler match: report missing %s\n' "$required" >&2
        exit 1
    fi
done
if [[ "$(awk -F= '/^participant_count=/{print $2}' "$report_file")" != "4" ]]; then
    printf 'brawler match: restarted roster did not retain four participants\n' >&2
    exit 1
fi
if [[ "$(awk -F= '/^summary_participant_count=/{print $2}' "$report_file")" != "4" \
    || "$(awk -F= '/^map_instance_id=/{print $2}' "$report_file")" -lt 1 \
    || "$(awk -F= '/^participant_active_ticks_team_1=/{print $2}' "$report_file")" -lt 1 \
    || "$(awk -F= '/^participant_active_ticks_team_2=/{print $2}' "$report_file")" -lt 1 ]]; then
    printf 'brawler match: summary identity or participant-time evidence is incomplete\n' >&2
    exit 1
fi
if [[ "$(awk -F= '/^defeats=/{print $2}' "$report_file")" -lt 1 \
    || "$(awk -F= '/^respawns=/{print $2}' "$report_file")" -lt 1 ]]; then
    printf 'brawler match: process run did not prove defeat and respawn\n' >&2
    exit 1
fi
if ((custom_build_client == 0)) && ! awk -F= '
    /^build_preset_ids=/ {
        count = split($2, ids, ",");
        for (i = 1; i <= count; i++) seen[ids[i]] = 1;
    }
    END { exit seen[1] && seen[2] && seen[3] && seen[4] ? 0 : 1 }
' "$report_file"; then
    printf 'brawler match: summary omitted the four named build presets\n' >&2
    exit 1
fi
if ((custom_build_client != 0)) \
    && [[ "$(awk -F= '/^custom_builds=/{print $2}' "$report_file")" -lt 1 ]]; then
    printf 'brawler match: summary omitted the custom build\n' >&2
    exit 1
fi
if [[ "$(awk -F= '/^accepted_attacks=/{print $2}' "$report_file")" -lt 1 ]]; then
    printf 'brawler match: summary omitted accepted attacks\n' >&2
    exit 1
fi
if [[ "$(awk -F= '/^dash_uses=/{print $2}' "$report_file")" -lt 1 \
    || "$(awk -F= '/^sentry_uses=/{print $2}' "$report_file")" -lt 1 \
    || "$(awk -F= '/^sentry_shots=/{print $2}' "$report_file")" -lt 1 ]]; then
    printf 'brawler match: process run did not exercise both ultimates and sentry fire\n' >&2
    exit 1
fi
if [[ -z "$(awk -F= '/^first_full_charge_ticks=/{print $2}' "$report_file")" \
    || -z "$(awk -F= '/^ability_uses_by_owner=/{print $2}' "$report_file")" ]]; then
    printf 'brawler match: per-owner charge/use evidence is incomplete\n' >&2
    exit 1
fi
if [[ "$(awk -F= '/^ability_dropped_records=/{print $2}' "$report_file")" != "0" \
    || "$(awk -F= '/^build_dropped_records=/{print $2}' "$report_file")" != "0" ]]; then
    printf 'brawler match: build or ability telemetry dropped records\n' >&2
    exit 1
fi
cat "$report_file"
