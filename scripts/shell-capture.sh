#!/usr/bin/env bash
# Native player-shell captures (BRL-pw9k): for each window size, start one scratch routed
# supervisor and run one automation client per route. Each client opens its route through the
# canonical flow actions (`--shell-capture <route>`), saves `<size>/<route>.png|json` once the
# screen has settled, and exits. Routes run in order against one profile, so `first-brawler`
# captures the empty-profile creation screen and creates the brawler the later routes show.
# Clients render off-screen (`--offscreen`, BRL-g7ny): no window appears and focus never moves.
#
# Live-match routes (BRL-wqst: match-loading, match-menu, leave-confirmation, results) play a real
# server-authoritative Practice match. Before the first of them the supervisor restarts on the same
# data with a capture catalog: a copy of config/server/game-types.ron whose matches last
# ${BRAWLER_SHELL_CAPTURE_MATCH_SECONDS:-30} s, so Results arrives on the authoritative time limit.
# BRAWLER_SHELL_CAPTURE_GAME_TYPE=<id> keeps only that game type, which Practice then selects
# (for example hot-zone-3v3 or heist-3v3 Results). Each live route leaves its match afterwards;
# `results` first plays again from Results to exercise the replay path.
#
# usage: scripts/shell-capture.sh [route|all] [WIDTHxHEIGHT|all]
# Output: ${BRAWLER_SHELL_CAPTURE_DIR:-target/shell-capture}/<size>/, with server data, client
# data, and per-route logs in <size>.run/ beside it. Never data/server/ or port 5000.
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

# Keep in the order of `ShellCaptureRoute::ALL` (packages/brawler-client/src/flow/automation.rs);
# the client rejects a name it does not know.
all_routes=(
    first-brawler dashboard dashboard-menu settings credits change-server
    brawler-list brawler-details brawler-create brawler-editor ultimate-picker
    weapon-equipment weapon-parts delete-brawler
    game-select practice-maps map-library map-editor map-editor-palette
    map-create map-editor-select map-editor-issues map-editor-test
    map-editor-rule map-editor-rule-show map-editor-focus map-editor-saved
    map-library-drafts map-rename map-delete queue
    match-loading match-menu leave-confirmation results
)
live_routes=" match-loading match-menu leave-confirmation results "
all_sizes=(1280x720 914x411)

route=${1:-all}
size=${2:-all}
out_root=${BRAWLER_SHELL_CAPTURE_DIR:-target/shell-capture}
bind_addr=${BRAWLER_SHELL_CAPTURE_BIND:-127.0.0.1:5031}
route_timeout_seconds=${BRAWLER_SHELL_CAPTURE_ROUTE_TIMEOUT_SECONDS:-90}
live_route_timeout_seconds=${BRAWLER_SHELL_CAPTURE_LIVE_ROUTE_TIMEOUT_SECONDS:-300}
match_seconds=${BRAWLER_SHELL_CAPTURE_MATCH_SECONDS:-30}
capture_game_type=${BRAWLER_SHELL_CAPTURE_GAME_TYPE:-}
if [[ ! "$match_seconds" =~ ^[1-9][0-9]*$ ]]; then
    printf '%s\n' 'brawler shell capture: match seconds must be a positive integer' >&2
    exit 2
fi
case "$bind_addr" in
    *:5000)
        printf '%s\n' 'brawler shell capture: refusing to bind port 5000, the developer server port' >&2
        exit 2
        ;;
esac
if [[ "$route" == all ]]; then
    routes=("${all_routes[@]}")
else
    routes=("$route")
fi
if [[ "$size" == all ]]; then
    sizes=("${all_sizes[@]}")
elif [[ "$size" =~ ^[0-9]+x[0-9]+$ ]]; then
    sizes=("$size")
else
    printf '%s\n' 'brawler shell capture: size must be WIDTHxHEIGHT or all' >&2
    exit 2
fi

target_dir=${CARGO_TARGET_DIR:-$project_dir/target}
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
    printf '%s\n' 'brawler shell capture: server returned an incomplete routing identity' >&2
    exit 1
fi

supervisor_pid=
client_pid=
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if [[ -n "$client_pid" ]] && kill -0 "$client_pid" 2>/dev/null; then
        kill -TERM "$client_pid" 2>/dev/null || true
        wait "$client_pid" 2>/dev/null || true
    fi
    stop_supervisor
    exit "$status"
}
stop_supervisor() {
    if [[ -n "$supervisor_pid" ]] && kill -0 "$supervisor_pid" 2>/dev/null; then
        kill -INT "$supervisor_pid" 2>/dev/null || true
        wait "$supervisor_pid" 2>/dev/null || true
    fi
    supervisor_pid=
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Writes the live-route capture catalog: production game types with bounded match durations,
# optionally only the requested game type. The server validates it like any operator catalog.
write_capture_catalog() {
    python3 - "$project_dir/config/server/game-types.ron" "$1" "$match_seconds" \
        "$capture_game_type" <<'PY'
import pathlib
import re
import sys

source, target, seconds, game_type = sys.argv[1:5]
text = pathlib.Path(source).read_text()
text, count = re.subn(r'match_duration_seconds:\s*\d+', f'match_duration_seconds: {seconds}', text)
if not count:
    raise SystemExit('capture catalog has no match durations')
if game_type:
    lines = text.splitlines(keepends=True)
    entries = [line for line in lines if line.lstrip().startswith('(id: "')]
    kept = [line for line in entries if line.lstrip().startswith(f'(id: "{game_type}"')]
    if len(kept) != 1:
        raise SystemExit(f'capture catalog has no game type {game_type}')
    text = ''.join(line for line in lines if line not in entries or line in kept)
pathlib.Path(target).write_text(text)
PY
}

# Starts this size's scratch supervisor on the given game-type catalog.
start_supervisor() {
    "$target_dir"/debug/brawler-supervisor \
        --network-protocol "$network_protocol" \
        --protocol-registry-fingerprint "$registry_fingerprint" \
        --content-fingerprint "$content_fingerprint" \
        --worker-executable "$target_dir/debug/brawler-server" \
        --game-types "$1" \
        --data-directory "$run_dir/server-data" \
        --mode wipeout \
        --match-rules production \
        --bind "$bind_addr" >>"$run_dir/supervisor.log" 2>&1 &
    supervisor_pid=$!
    sleep 1
}

# Runs one client to completion within the given timeout; returns its exit status.
run_client() {
    local log=$1
    local route_timeout_seconds=$2
    shift 2
    "$@" >"$log" 2>&1 &
    client_pid=$!
    local waited=0
    while kill -0 "$client_pid" 2>/dev/null; do
        if ((waited >= route_timeout_seconds * 10)); then
            printf 'brawler shell capture: client timed out after %ss; log=%s\n' \
                "$route_timeout_seconds" "$log" >&2
            kill -TERM "$client_pid" 2>/dev/null || true
            wait "$client_pid" 2>/dev/null || true
            client_pid=
            return 1
        fi
        sleep 0.1
        waited=$((waited + 1))
    done
    local status=0
    wait "$client_pid" || status=$?
    client_pid=
    # A panic inside the macOS event loop can unwind without a non-zero status.
    if grep -q 'panicked at' "$log"; then
        status=1
    fi
    return "$status"
}

for window_size in "${sizes[@]}"; do
    capture_dir=$out_root/$window_size
    run_dir=$out_root/$window_size.run
    # Each size starts from a fresh scratch server and client profile owned by this script.
    rm -rf "$capture_dir" "$run_dir"
    mkdir -p "$capture_dir" "$run_dir"
    start_supervisor "$project_dir/config/server/game-types.ron"
    live_catalog=
    for name in "${routes[@]}"; do
        log=$run_dir/$name.log
        timeout=$route_timeout_seconds
        if [[ "$live_routes" == *" $name "* ]]; then
            timeout=$live_route_timeout_seconds
            if [[ -z "$live_catalog" ]]; then
                # Same server data, so the profile the earlier routes created carries over.
                live_catalog=$run_dir/capture-game-types.ron
                write_capture_catalog "$live_catalog"
                stop_supervisor
                start_supervisor "$live_catalog"
            fi
        fi
        if ! run_client "$log" "$timeout" env BRAWLER_CLIENT_DATA_DIR="$run_dir/client-data" \
            "$target_dir"/release/brawler-client --server "$bind_addr" \
            --window-size "$window_size" --screenshot-dir "$capture_dir" \
            --shell-capture "$name" --offscreen; then
            printf 'brawler shell capture: route %s at %s failed; log=%s\n' \
                "$name" "$window_size" "$log" >&2
            exit 1
        fi
        for extension in png json; do
            if [[ ! -f "$capture_dir/$name.$extension" ]]; then
                printf 'brawler shell capture: route %s at %s saved no %s; log=%s\n' \
                    "$name" "$window_size" "$extension" "$log" >&2
                exit 1
            fi
        done
        printf 'brawler shell capture: %s %s\n' "$window_size" "$name"
    done
    stop_supervisor
done
printf 'brawler shell capture: passed; captures=%s\n' "$out_root"
