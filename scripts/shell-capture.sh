#!/usr/bin/env bash
# Native player-shell captures (BRL-pw9k): for each window size, start one scratch routed
# supervisor and run one automation client per route. Each client opens its route through the
# canonical flow actions (`--shell-capture <route>`), saves `<size>/<route>.png|json` once the
# screen has settled, and exits. Routes run in order against one profile, so `first-brawler`
# captures the empty-profile creation screen and creates the brawler the later routes show.
# Clients render off-screen (`--offscreen`, BRL-g7ny): no window appears and focus never moves.
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
    map-create map-editor-select map-editor-issues map-editor-test map-editor-saved
    map-library-drafts map-rename map-delete queue
)
all_sizes=(1280x720 914x411)

route=${1:-all}
size=${2:-all}
out_root=${BRAWLER_SHELL_CAPTURE_DIR:-target/shell-capture}
bind_addr=${BRAWLER_SHELL_CAPTURE_BIND:-127.0.0.1:5031}
route_timeout_seconds=${BRAWLER_SHELL_CAPTURE_ROUTE_TIMEOUT_SECONDS:-90}
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

# Runs one client to completion within the route timeout; returns its exit status.
run_client() {
    local log=$1
    shift
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
    "$target_dir"/debug/brawler-supervisor \
        --network-protocol "$network_protocol" \
        --protocol-registry-fingerprint "$registry_fingerprint" \
        --content-fingerprint "$content_fingerprint" \
        --worker-executable "$target_dir/debug/brawler-server" \
        --game-types "$project_dir/config/server/game-types.ron" \
        --data-directory "$run_dir/server-data" \
        --mode wipeout \
        --match-rules production \
        --bind "$bind_addr" >"$run_dir/supervisor.log" 2>&1 &
    supervisor_pid=$!
    sleep 1
    for name in "${routes[@]}"; do
        log=$run_dir/$name.log
        if ! run_client "$log" env BRAWLER_CLIENT_DATA_DIR="$run_dir/client-data" \
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
