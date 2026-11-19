#!/usr/bin/env bash
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
report_path=${1:-target/v3-render-evidence.txt}
peer_report_path=${report_path}.peer
capture_dir=${report_path}.captures
# Supervisor and client data for this run stay beside the report, never in data/server/ or the
# developer's client data directory. Binaries come from ${CARGO_TARGET_DIR:-target}.
server_data_dir=${report_path}.server-data
client_data_dir=${report_path}.client-data
bind_addr=${BRAWLER_RENDER_BIND:-127.0.0.1:5024}
case "$bind_addr" in
    *:5000)
        printf '%s\n' 'brawler render evidence: refusing to bind port 5000, the developer server port' >&2
        exit 2
        ;;
esac
game_mode=${BRAWLER_RENDER_MODE:-wipeout}
players_per_team=${BRAWLER_RENDER_PLAYERS_PER_TEAM:-}
game_type=${BRAWLER_RENDER_GAME_TYPE:-}
timeout_seconds=${BRAWLER_RENDER_TIMEOUT_SECONDS:-75}
warmup_seconds=${BRAWLER_RENDER_WARMUP_SECONDS:-}
measure_seconds=${BRAWLER_RENDER_MEASURE_SECONDS:-}
window_size=${BRAWLER_RENDER_WINDOW_SIZE:-1280x720}
peer_render_profile=${BRAWLER_RENDER_PEER_PROFILE:-native}
validate_peer=${BRAWLER_RENDER_VALIDATE_PEER:-1}
practice_smoke=${BRAWLER_RENDER_PRACTICE:-0}
reduced_effects=${BRAWLER_RENDER_REDUCED_EFFECTS:-0}
combat_smoke=${BRAWLER_RENDER_COMBAT:-0}
# Optional primary weapon for the measured client: a costed weapon ID from
# content/catalogs/builds.ron. The client validates it against the embedded catalog and selects,
# or creates and selects, a saved brawler with that weapon through the canonical profile commands.
render_weapon=${BRAWLER_RENDER_WEAPON:-}
# Optional fixed aim for the measured client only (BRL-wegn): up, down, left, right, or a signed
# X,Y axis. It replaces the combat-smoke aim (the peer keeps its own) so the captures show the
# camera look-ahead toward one screen edge. Simulation +Y renders toward the top of the screen
# (coordinates.rs maps it to render -Z; camera.rs looks along -Z from +Z with +Y up).
render_aim=${BRAWLER_RENDER_AIM:-}
# Optional measured-client move axis (signed X,Y). Defaults to 1,0, or 0,0 while an aim is held
# so the lean is not mixed with follow motion.
render_move=${BRAWLER_RENDER_MOVE:-}
client_one_log=${report_path}.client-1.log
client_two_log=${report_path}.client-2.log
# The measured client requests exactly this many paired frame/state captures.
capture_count=3

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
case "$practice_smoke" in
    0 | 1) ;;
    *)
        printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_PRACTICE must be 0 or 1' >&2
        exit 2
        ;;
esac
case "$reduced_effects" in
    0) render_effect_flag= ;;
    1) render_effect_flag=--render-reduced-effects ;;
    *)
        printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_REDUCED_EFFECTS must be 0 or 1' >&2
        exit 2
        ;;
esac
case "$combat_smoke" in
    0) measured_aim_flags=; measured_action_flags=; peer_combat_flags= ;;
    1)
        measured_aim_flags='--aim-axis 1,0'
        measured_action_flags='--fire --ultimate'
        peer_combat_flags='--aim-axis -1,0 --fire --ultimate'
        ;;
    dummy)
        # Chase and shoot the nearest opponent (a Practice bot), using the ultimate (Dash for
        # the default build) whenever it is charged: exercises followed-authority motion.
        measured_aim_flags='--aim-dummy'
        measured_action_flags='--fire --ultimate'
        peer_combat_flags='--aim-dummy --fire --ultimate'
        ;;
    *)
        printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_COMBAT must be 0, 1, or dummy' >&2
        exit 2
        ;;
esac
case "$render_weapon" in
    '') measured_weapon_flags= ;;
    *[!0-9]* | 0*)
        printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_WEAPON must be a build-catalog weapon ID' >&2
        exit 2
        ;;
    *) measured_weapon_flags="--weapon-preset $render_weapon" ;;
esac
# Accept a signed X,Y pair whose components fit the client's i8 axis parser.
is_axis() {
    local value=$1 x y
    [[ "$value" =~ ^-?[0-9]{1,3},-?[0-9]{1,3}$ ]] || return 1
    x=${value%%,*}
    y=${value#*,}
    ((10#${x#-} <= 127 && 10#${y#-} <= 127))
}
case "$render_aim" in
    '') measured_aim_axis= ;;
    up) measured_aim_axis=0,1 ;;
    down) measured_aim_axis=0,-1 ;;
    left) measured_aim_axis=-1,0 ;;
    right) measured_aim_axis=1,0 ;;
    *)
        if ! is_axis "$render_aim" || [[ "$render_aim" =~ ^-?0+,-?0+$ ]]; then
            printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_AIM must be up, down, left, right, or a non-zero signed X,Y' >&2
            exit 2
        fi
        measured_aim_axis=$render_aim
        ;;
esac
if [[ -n "$measured_aim_axis" ]]; then
    measured_aim_flags="--aim-axis $measured_aim_axis"
fi
measured_combat_flags="$measured_aim_flags${measured_aim_flags:+${measured_action_flags:+ }}$measured_action_flags"
if [[ -n "$render_move" ]]; then
    if ! is_axis "$render_move"; then
        printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_MOVE must be a signed X,Y axis' >&2
        exit 2
    fi
    measured_move_axis=$render_move
elif [[ -n "$measured_aim_axis" ]]; then
    measured_move_axis=0,0
else
    measured_move_axis=1,0
fi
if [[ "$practice_smoke" == 1 ]]; then
    validate_peer=0
fi
if [[ -z "$players_per_team" ]]; then
    if [[ "$game_mode" == hot-zone ]]; then players_per_team=2; else players_per_team=1; fi
fi
# The report fails a window that leaves the measured match (left_match_during_measurement,
# BRL-rr1x), so every default window must end inside a live match. Heist Practice bots break a
# safe and end the 180 s match as early as 18 s after match-ready (observed 18-23 s with the
# measured client walking and firing right, and with it aiming elsewhere; some runs last past
# 40 s). The window is anchored at match readiness and includes the 3 s countdown, so Heist
# Practice defaults to 4 s warmup + 12 s measurement (16 s). Other modes keep 10 s + 30 s.
# Explicit durations override both. The client configuration check below rejects a measurement
# too short for the 1,200-sample gate at the run's cadence with 20% headroom (BRL-vh61): 12 s for
# the native profile off-screen (paced at 120 Hz, measured 110-114 Hz), 24 s at 60 Hz, 48 s at
# 30 Hz.
if [[ "$practice_smoke" == 1 && "$game_mode" == heist ]]; then
    warmup_seconds=${warmup_seconds:-4}
    measure_seconds=${measure_seconds:-12}
else
    warmup_seconds=${warmup_seconds:-10}
    measure_seconds=${measure_seconds:-30}
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
if [[ "$practice_smoke" == 1 ]]; then
    roster_size=1
else
    roster_size=$((players_per_team * 2))
fi
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
target_dir=${CARGO_TARGET_DIR:-$project_dir/target}
if [[ -e "$report_path" || -e "$peer_report_path" || -e "$client_one_log" || -e "$client_two_log" || -e "$capture_dir" || -e "$server_data_dir" || -e "$client_data_dir" ]]; then
    printf '%s\n' 'brawler render evidence: refusing to overwrite an existing report, client log, or capture directory' >&2
    exit 2
fi

commit=$(git rev-parse --short=12 HEAD 2>/dev/null || printf unknown)
BRAWLER_GIT_COMMIT="$commit" cargo build --locked --release -p brawler-client --bin brawler-client \
    --features automation
cargo build --locked --bin brawler-server --no-default-features --features server
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$("$target_dir"/debug/brawler-server routing-identity)
network_protocol=$(awk -F= '$1 == "network_protocol" { print $2 }' <<<"$identity")
registry_fingerprint=$(awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }' <<<"$identity")
content_fingerprint=$(awk -F= '$1 == "content_fingerprint" { print $2 }' <<<"$identity")
if [[ -z "$network_protocol" || -z "$registry_fingerprint" || -z "$content_fingerprint" ]]; then
    printf '%s\n' 'brawler render evidence: server returned an incomplete routing identity' >&2
    exit 1
fi

# The measured and peer client command lines, shared by the configuration check below and the
# launch. The optional flag strings split into words on purpose.
if [[ "$practice_smoke" == 1 ]]; then
    measured_match_flag=--product-practice-smoke
else
    measured_match_flag=$match_flag
fi
measured_args=(--client-id 1 --server "$bind_addr" --transport routed-udp
    --auto-connect "$measured_match_flag" --product-game-type "$game_type"
    --move-axis "$measured_move_axis" --window-size "$window_size" --offscreen
    --screenshot-dir "$capture_dir" --screenshot-first 300 --screenshot-every 180
    --screenshot-count "$capture_count"
    --render-report "$report_path" --render-warmup-seconds "$warmup_seconds"
    --render-measure-seconds "$measure_seconds")
measured_args+=($render_effect_flag $measured_combat_flags $measured_weapon_flags)
peer_args=(--client-id 2 --server "$bind_addr" --transport routed-udp
    --auto-connect "$match_flag" --product-game-type "$game_type" --move-axis '-1,0'
    --window-size "$window_size" --offscreen
    --render-report "$peer_report_path" --render-warmup-seconds "$warmup_seconds"
    --render-measure-seconds "$measure_seconds")
peer_args+=($render_effect_flag $peer_combat_flags)

# Reject a configuration the clients would refuse, notably a measurement window too short for the
# report's 1,200-sample gate at the run's cadence (BRL-vh61), before the routed topology starts.
check_client_config() {
    local role=$1
    shift
    if ! "$@" --check-config >/dev/null; then
        printf 'brawler render evidence: the %s client rejected its configuration (above); set BRAWLER_RENDER_MEASURE_SECONDS to at least the duration it names\n' \
            "$role" >&2
        exit 2
    fi
}
check_client_config measured "$target_dir"/release/brawler-client "${measured_args[@]}"
if [[ "$practice_smoke" == 0 ]]; then
    check_client_config peer env BRAWLER_RENDER_PROFILE="$peer_render_profile" \
        "$target_dir"/release/brawler-client "${peer_args[@]}"
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

"$target_dir"/debug/brawler-supervisor \
    --network-protocol "$network_protocol" \
    --protocol-registry-fingerprint "$registry_fingerprint" \
    --content-fingerprint "$content_fingerprint" \
    --worker-executable "$target_dir/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --data-directory "$server_data_dir" \
    --mode "$game_mode" \
    --match-rules production \
    --bind "$bind_addr" &
supervisor_pid=$!

# Both clients render off-screen (`--offscreen`, BRL-g7ny): no window, no focus change. Their
# frame interval follows the client's schedule runner (120 Hz for the native profile), not
# display presentation; the report records `render_target=offscreen`.
BRAWLER_CLIENT_DATA_DIR="$client_data_dir/1" "$target_dir"/release/brawler-client "${measured_args[@]}" \
    >"$client_one_log" 2>&1 &
measured_pid=$!
# Match the canonical routed-product smoke: do not turn a two-client evidence run into a burst
# against the deliberately small unauthenticated lobby ingress budget.
sleep 2
if [[ "$practice_smoke" == 0 ]]; then
    env BRAWLER_RENDER_PROFILE="$peer_render_profile" BRAWLER_CLIENT_DATA_DIR="$client_data_dir/2" \
        "$target_dir"/release/brawler-client "${peer_args[@]}" >"$client_two_log" 2>&1 &
    peer_pid=$!
fi

for ((client_id = 3; client_id <= roster_size; client_id++)); do
    sleep 1
    BRAWLER_CLIENT_DATA_DIR="$client_data_dir/$client_id" "$target_dir"/release/brawler-client --client-id "$client_id" --server "$bind_addr" \
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

measured_status=0
wait "$measured_pid" || measured_status=$?
measured_pid=
kill "$watchdog_pid" 2>/dev/null || true
wait "$watchdog_pid" 2>/dev/null || true
watchdog_pid=

if [[ "$measured_status" -ne 0 ]]; then
    printf 'brawler render evidence: measured client exited with status %s; log=%s\n' \
        "$measured_status" "$client_one_log" >&2
    exit 1
fi
# A panic inside the macOS event loop can unwind out of winit without a non-zero exit status,
# so the client log is the authoritative signal that a client crashed during the run.
for client_log in "$client_one_log" "$client_two_log" "${report_path}".client-[0-9]*.log; do
    if [[ -f "$client_log" ]] && grep -q 'panicked at' "$client_log"; then
        printf 'brawler render evidence: client panicked; log=%s\n' "$client_log" >&2
        exit 1
    fi
done

completed_reports=("$report_path")
if [[ "$practice_smoke" == 0 && "$validate_peer" == 1 ]]; then
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
png_count=$({ find "$capture_dir" -maxdepth 1 -name 'brawler-*.png' -type f 2>/dev/null || true; } | wc -l | tr -d ' ')
json_count=$({ find "$capture_dir" -maxdepth 1 -name 'brawler-*.json' -type f 2>/dev/null || true; } | wc -l | tr -d ' ')
if [[ "$png_count" -ne "$capture_count" || "$json_count" -ne "$capture_count" ]]; then
    printf 'brawler render evidence: expected %s PNG and JSON captures, found %s PNG and %s JSON in %s\n' \
        "$capture_count" "$png_count" "$json_count" "$capture_dir" >&2
    exit 1
fi
aim_summary=
if [[ -n "$measured_aim_axis" ]]; then
    aim_summary="; aim=${render_aim} (${measured_aim_axis}); move=${measured_move_axis}"
fi
if [[ "$practice_smoke" == 1 ]]; then
    printf 'brawler render evidence: Practice %s passed; report=%s; captures=%s%s\n' \
        "$game_type" "$report_path" "$capture_dir" "$aim_summary"
elif [[ "$validate_peer" == 1 ]]; then
    printf 'brawler render evidence: passed; reports=%s,%s; captures=%s%s\n' \
        "$report_path" "$peer_report_path" "$capture_dir" "$aim_summary"
else
    printf 'brawler render evidence: passed; report=%s; pacing-peer=%s; captures=%s%s\n' \
        "$report_path" "$peer_report_path" "$capture_dir" "$aim_summary"
fi
