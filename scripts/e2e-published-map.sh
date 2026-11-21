#!/usr/bin/env bash
set -euo pipefail

# BRL-w9z1 routed publish-to-rotation check. Headless client A publishes a copy of the
# takedown-1v1 built-in map to a fresh routed server, the operator CLI approves it, and two exact
# 1v1 queue matches follow: the first plays the built-in map, the second the published map, which
# both clients must load (and see credited on Match Loading) before the match reaches Active.
# Then (BRL-s3dh) client B cannot withdraw client A's map, client A lists it as approved and in
# rotation and withdraws it, and the operator CLI shows it removed.

game_type=takedown-1v1
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

if [[ -z "${BRAWLER_ROUTED_BIND:-}" ]]; then
    port=$(python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
    )
    BRAWLER_ROUTED_BIND="127.0.0.1:$port"
fi
bind_addr=$BRAWLER_ROUTED_BIND
# A fresh store keeps published-map IDs and the rotation order deterministic.
server_data_dir=${BRAWLER_SERVER_DATA_DIR:-$project_dir/target/e2e/published-map}
timeout_seconds=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-90}
log_dir="$server_data_dir.logs"
rm -rf "$server_data_dir" "$log_dir"
mkdir -p "$log_dir"

supervisor_pid=
watchdog_pid=
client_pids=()

cleanup() {
    trap - INT TERM EXIT
    for pid in "$watchdog_pid" ${client_pids[@]+"${client_pids[@]}"}; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    if [[ -n "$supervisor_pid" ]] && kill -0 "$supervisor_pid" 2>/dev/null; then
        kill -INT "$supervisor_pid" 2>/dev/null || true
        wait "$supervisor_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT
trap 'cleanup; exit 124' INT TERM

fail() {
    printf 'brawler published-map e2e: %s (logs in %s)\n' "$1" "$log_dir" >&2
    exit 1
}

cargo build --locked -p brawler-client --bin brawler-client --features automation
cargo build --locked --bin brawler-server --bin brawler-profile-admin --no-default-features --features server
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$(target/debug/brawler-server routing-identity)
field() { printf '%s\n' "$identity" | awk -F= -v key="$1" '$1 == key { print $2 }'; }

printf 'brawler published-map e2e: routed server on %s, data in %s\n' "$bind_addr" "$server_data_dir"
target/debug/brawler-supervisor \
    --network-protocol "$(field network_protocol)" \
    --protocol-registry-fingerprint "$(field protocol_registry_fingerprint)" \
    --content-fingerprint "$(field content_fingerprint)" \
    --worker-executable "$project_dir/target/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --data-directory "$server_data_dir" \
    --bind "$bind_addr" >"$log_dir/supervisor.log" 2>&1 &
supervisor_pid=$!

(
    sleep "$timeout_seconds"
    echo "brawler published-map e2e: timed out after ${timeout_seconds}s" >&2
    kill -TERM $$
) &
watchdog_pid=$!

client() {
    local client_id=$1 log=$2
    shift 2
    target/debug/brawler-client \
        --client-id "$client_id" \
        --server "$bind_addr" \
        --transport routed-udp \
        --auto-connect \
        --headless \
        --product-game-type "$game_type" \
        "$@" >"$log_dir/$log.log" 2>&1
}

# Run one exact 1v1 queue match with clients A and B; extra flags apply to both.
queue_match() {
    local label=$1
    shift
    client 7101 "$label-a" --product-match-smoke-1v1 --character 1 "$@" &
    client_pids=($!)
    sleep 2
    client 7102 "$label-b" --product-match-smoke-1v1 --character 1 "$@" &
    client_pids+=($!)
    local status=0
    for pid in "${client_pids[@]}"; do
        wait "$pid" || status=$?
    done
    client_pids=()
    [[ "$status" -eq 0 ]] || fail "$label match client failed with status $status"
}

sleep 2
client 7101 publish --product-publish-map-smoke || fail "client A could not publish"
published_id=$(awk '/brawler-client published-map id=/ { sub(/.*id=/, ""); print $1; exit }' "$log_dir/publish.log")
[[ -n "$published_id" ]] || fail "client A reported no published map"
grep -q "published-map id=$published_id state=Pending" "$log_dir/publish.log" \
    || fail "the checked-in server must hold a new map for review"
printf 'brawler published-map e2e: client A published map %s (pending)\n' "$published_id"

database="$server_data_dir/profiles.sqlite3"
target/debug/brawler-profile-admin maps list --database "$database" | tee "$log_dir/maps-pending.txt"
target/debug/brawler-profile-admin maps approve "$published_id" --database "$database"
# The lobby re-reads approvals on its 5 s interval.
sleep 6

queue_match built-in
grep -q 'match-map recipe_id=' "$log_dir/built-in-a.log" || fail "the first match reported no map"
if grep -q 'match-loading credit=' "$log_dir/built-in-a.log"; then
    fail "the first rotation slot must be the built-in map"
fi
printf 'brawler published-map e2e: first 1v1 match played the built-in map\n'

queue_match published --expect-published-map
for side in a b; do
    grep -q "match-loading credit=\"E2E Published Yard — by .*\" published_map_id=$published_id" \
        "$log_dir/published-$side.log" || fail "client $side saw no Match Loading credit"
    grep -q 'match-map recipe_id=' "$log_dir/published-$side.log" \
        || fail "client $side reported no published map"
done
grep -h 'match-loading credit=' "$log_dir/published-a.log"
printf 'brawler published-map e2e: published map %s loaded for clients A and B and reached Active\n' "$published_id"

# BRL-s3dh: only the author lists and withdraws the map. Client B's withdraw is refused as
# not-found; client A sees it approved and in rotation, withdraws it, and the operator store
# shows it removed with the author's reason.
if client 7102 withdraw-foreign --product-published-maps-smoke --withdraw-published-map "$published_id"; then
    fail "client B withdrew client A's map"
fi
grep -q 'NotFound' "$log_dir/withdraw-foreign.log" || fail "client B's withdraw was not refused as not found"
client 7101 list --product-published-maps-smoke || fail "client A could not list its published maps"
grep -q "published-map-row id=$published_id state=approved-in-rotation" "$log_dir/list.log" \
    || fail "client A's map is not listed as approved and in rotation"
client 7101 withdraw --product-published-maps-smoke --withdraw-published-map "$published_id" \
    || fail "client A could not withdraw its map"
grep -q "published-map-row id=$published_id state=removed" "$log_dir/withdraw.log" \
    || fail "client A's withdrawn map is not listed as removed"
grep -h 'published-map-row' "$log_dir/list.log" "$log_dir/withdraw.log"
target/debug/brawler-profile-admin maps list --state removed --database "$database" | tee "$log_dir/maps-withdrawn.txt"
grep -q "^$published_id	removed	.*Withdrawn by the author" "$log_dir/maps-withdrawn.txt" || fail "the operator store does not show the withdrawal"
printf 'brawler published-map e2e: client A listed and withdrew map %s; client B could not\n' "$published_id"
