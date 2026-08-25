#!/usr/bin/env bash
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
report_path=${1:-target/v3-render-evidence.txt}
peer_report_path=${report_path}.peer
bind_addr=${BRAWLER_RENDER_BIND:-127.0.0.1:5024}
game_mode=${BRAWLER_RENDER_MODE:-wipeout}
players_per_team=${BRAWLER_RENDER_PLAYERS_PER_TEAM:-}
game_type=${BRAWLER_RENDER_GAME_TYPE:-}
timeout_seconds=${BRAWLER_RENDER_TIMEOUT_SECONDS:-75}
warmup_seconds=${BRAWLER_RENDER_WARMUP_SECONDS:-10}
measure_seconds=${BRAWLER_RENDER_MEASURE_SECONDS:-30}
window_size=${BRAWLER_RENDER_WINDOW_SIZE:-1280x720}
peer_render_profile=${BRAWLER_RENDER_PEER_PROFILE:-native}
validate_peer=${BRAWLER_RENDER_VALIDATE_PEER:-1}
client_one_log=${report_path}.client-1.log
client_two_log=${report_path}.client-2.log

case "$game_mode" in
    wipeout | hot-zone | heist) ;;
    *)
        printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_MODE must be wipeout, hot-zone, or heist' >&2
        exit 2
        ;;
esac
case "$validate_peer" in
    0 | 1) ;;
    *)
        printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_VALIDATE_PEER must be 0 or 1' >&2
        exit 2
        ;;
esac
if [[ -z "$players_per_team" ]]; then
    if [[ "$game_mode" == hot-zone ]]; then players_per_team=2; else players_per_team=1; fi
fi
if [[ -z "$game_type" ]]; then
    if [[ "$game_mode" == hot-zone ]]; then
        game_type=hot-zone-2v2
    elif [[ "$game_mode" == heist ]]; then
        game_type="heist-${players_per_team}v${players_per_team}"
    else
        game_type=wipeout-1v1
    fi
fi
case "$players_per_team" in
    1) match_flag=--product-match-smoke-1v1 ;;
    2) match_flag=--product-match-smoke ;;
    3) match_flag=--product-match-smoke-3v3 ;;
    *)
        printf '%s\n' 'brawler render evidence: players per team must be 1, 2, or 3' >&2
        exit 2
        ;;
esac
roster_size=$((players_per_team * 2))
case "$timeout_seconds" in
    '' | *[!0-9]* | 0)
        printf '%s\n' 'brawler render evidence: timeout must be a positive integer' >&2
        exit 2
        ;;
esac
for duration in "$warmup_seconds" "$measure_seconds"; do
    case "$duration" in
        '' | *[!0-9]* | 0)
            printf '%s\n' 'brawler render evidence: durations must be positive integers' >&2
            exit 2
            ;;
    esac
done

cd "$project_dir"
if [[ -e "$report_path" || -e "$peer_report_path" || -e "$client_one_log" || -e "$client_two_log" ]]; then
    printf '%s\n' 'brawler render evidence: refusing to overwrite an existing report or client log' >&2
    exit 2
fi

commit=$(git rev-parse --short=12 HEAD 2>/dev/null || printf unknown)
BRAWLER_GIT_COMMIT="$commit" cargo build --locked --release --bin brawler-client \
    --no-default-features --features client
cargo build --locked --bin brawler-server --no-default-features --features server
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$(target/debug/brawler-server routing-identity)
network_protocol=$(awk -F= '$1 == "network_protocol" { print $2 }' <<<"$identity")
registry_fingerprint=$(awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }' <<<"$identity")
content_fingerprint=$(awk -F= '$1 == "content_fingerprint" { print $2 }' <<<"$identity")
if [[ -z "$network_protocol" || -z "$registry_fingerprint" || -z "$content_fingerprint" ]]; then
    printf '%s\n' 'brawler render evidence: server returned an incomplete routing identity' >&2
    exit 1
fi

supervisor_pid=
measured_pid=
peer_pid=
# Bash 3.2 treats an empty array expansion as unbound under `set -u`. Keep one harmless empty
# sentinel so the common 1v1 evidence path and the 2v2 auxiliary-client path share safe cleanup.
extra_pids=("")
watchdog_pid=
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if [[ -n "$watchdog_pid" ]] && kill -0 "$watchdog_pid" 2>/dev/null; then
        kill "$watchdog_pid" 2>/dev/null || true
    fi
    for pid in "$measured_pid" "$peer_pid" "${extra_pids[@]}"; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    if [[ -n "$supervisor_pid" ]] && kill -0 "$supervisor_pid" 2>/dev/null; then
        kill -INT "$supervisor_pid" 2>/dev/null || true
    fi
    for pid in "$measured_pid" "$peer_pid" "${extra_pids[@]}" "$supervisor_pid"; do
        if [[ -n "$pid" ]]; then
            wait "$pid" 2>/dev/null || true
        fi
    done
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

target/debug/brawler-supervisor \
    --network-protocol "$network_protocol" \
    --protocol-registry-fingerprint "$registry_fingerprint" \
    --content-fingerprint "$content_fingerprint" \
    --worker-executable "$project_dir/target/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --mode "$game_mode" \
    --match-rules production \
    --bind "$bind_addr" &
supervisor_pid=$!

target/release/brawler-client --client-id 1 --server "$bind_addr" --transport routed-udp \
    --auto-connect "$match_flag" --product-game-type "$game_type" --move-axis 1,0 \
    --window-size "$window_size" \
    --render-report "$report_path" --render-warmup-seconds "$warmup_seconds" \
    --render-measure-seconds "$measure_seconds" \
    >"$client_one_log" 2>&1 &
measured_pid=$!
# Match the canonical routed-product smoke: do not turn a two-client evidence run into a burst
# against the deliberately small unauthenticated lobby ingress budget.
sleep 2
env BRAWLER_RENDER_PROFILE="$peer_render_profile" \
    target/release/brawler-client --client-id 2 --server "$bind_addr" --transport routed-udp \
    --auto-connect "$match_flag" --product-game-type "$game_type" --move-axis '-1,0' \
    --window-size "$window_size" \
    --render-report "$peer_report_path" --render-warmup-seconds "$warmup_seconds" \
    --render-measure-seconds "$measure_seconds" \
    >"$client_two_log" 2>&1 &
peer_pid=$!

for ((client_id = 3; client_id <= roster_size; client_id++)); do
    sleep 1
    target/release/brawler-client --client-id "$client_id" --server "$bind_addr" \
        --transport routed-udp --headless "$match_flag" \
        --product-game-type "$game_type" \
        >"${report_path}.client-${client_id}.log" 2>&1 &
    extra_pids+=("$!")
done

(
    sleep "$timeout_seconds"
    printf 'brawler render evidence: timed out after %ss\n' "$timeout_seconds" >&2
    kill -TERM "$$"
) &
watchdog_pid=$!

wait "$measured_pid"
measured_pid=
kill "$watchdog_pid" 2>/dev/null || true
wait "$watchdog_pid" 2>/dev/null || true
watchdog_pid=

completed_reports=("$report_path")
if [[ "$validate_peer" == 1 ]]; then
    completed_reports+=("$peer_report_path")
fi
for completed_report in "${completed_reports[@]}"; do
    if [[ ! -f "$completed_report" ]] || \
        ! awk -F= '$1 == "result" && $2 == "pass" { passed = 1 } END { exit !passed }' \
        "$completed_report"; then
        printf 'brawler render evidence: report failed its locked threshold: %s\n' \
            "$completed_report" >&2
        exit 1
    fi
done
if [[ "$validate_peer" == 1 ]]; then
    printf 'brawler render evidence: passed; reports=%s,%s\n' "$report_path" "$peer_report_path"
else
    printf 'brawler render evidence: passed; report=%s; pacing-peer=%s\n' \
        "$report_path" "$peer_report_path"
fi
