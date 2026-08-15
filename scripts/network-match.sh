#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
    target_dir="$repo_root/$target_dir"
fi
network_addr="${BRAWLER_NETWORK_ADDR:-127.0.0.1:5200}"
weapon_preset="${BRAWLER_NETWORK_WEAPON_PRESET:-1}"
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
    "$target_dir/debug/brawler-server" --bind "$network_addr") &
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
    (trap '' INT; exec "$target_dir/debug/brawler-client" \
        --server "$network_addr" --client-id "$client_id" --headless \
        --exit-after-roster 4 --simulation-ticks 2000 --weapon-preset "$weapon_preset" \
        --move-axis "$move_axis" --aim-dummy --fire) &
    client_pids+=("$!")
done

deadline=$((SECONDS + 60))
while job_is_running "$server_pid"; do
    if ((SECONDS >= deadline)); then
        printf 'brawler match: shortened 2v2 match timed out\n' >&2
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

for required in initial_match_id restarted_match_id participant_count final_score_team_1 \
    summary_participant_count map_instance_id map_recipe_fingerprint content_fingerprint rules_revision \
    final_score_team_2 result active_duration_ticks defeats respawns participant_active_ticks_team_1 \
    participant_active_ticks_team_2 records dropped_records summary_count \
    weapon_aggregate_count weapon_preset_ids accepted_attacks attacks_with_hostile_contact; do
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
if ! awk -F= -v preset="$weapon_preset" '
    /^weapon_preset_ids=/ {
        count = split($2, ids, ",");
        for (i = 1; i <= count; i++) if (ids[i] == preset) found = 1;
    }
    END { exit found ? 0 : 1 }
' "$report_file"; then
    printf 'brawler match: summary omitted weapon preset %s\n' "$weapon_preset" >&2
    exit 1
fi
if [[ "$(awk -F= '/^accepted_attacks=/{print $2}' "$report_file")" -lt 1 ]]; then
    printf 'brawler match: summary omitted accepted attacks\n' >&2
    exit 1
fi
cat "$report_file"
