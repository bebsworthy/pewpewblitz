#!/usr/bin/env bash
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
report_path=${1:-target/v3-render-evidence.txt}
peer_report_path=${report_path}.peer
bind_addr=${BRAWLER_RENDER_BIND:-127.0.0.1:5024}
game_mode=${BRAWLER_RENDER_MODE:-wipeout}
timeout_seconds=${BRAWLER_RENDER_TIMEOUT_SECONDS:-75}
client_one_log=${report_path}.client-1.log
client_two_log=${report_path}.client-2.log

case "$game_mode" in
    wipeout | hot-zone) ;;
    *)
        printf '%s\n' 'brawler render evidence: BRAWLER_RENDER_MODE must be wipeout or hot-zone' >&2
        exit 2
        ;;
esac
case "$timeout_seconds" in
    '' | *[!0-9]* | 0)
        printf '%s\n' 'brawler render evidence: timeout must be a positive integer' >&2
        exit 2
        ;;
esac

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
watchdog_pid=
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if [[ -n "$watchdog_pid" ]] && kill -0 "$watchdog_pid" 2>/dev/null; then
        kill "$watchdog_pid" 2>/dev/null || true
    fi
    for pid in "$measured_pid" "$peer_pid"; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    if [[ -n "$supervisor_pid" ]] && kill -0 "$supervisor_pid" 2>/dev/null; then
        kill -INT "$supervisor_pid" 2>/dev/null || true
    fi
    for pid in "$measured_pid" "$peer_pid" "$supervisor_pid"; do
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
    --auto-connect --product-match-smoke-1v1 --controller-demo --window-size 1280x720 \
    --render-report "$report_path" \
    >"$client_one_log" 2>&1 &
measured_pid=$!
target/release/brawler-client --client-id 2 --server "$bind_addr" --transport routed-udp \
    --auto-connect --product-match-smoke-1v1 --controller-demo --window-size 1280x720 \
    --render-report "$peer_report_path" \
    >"$client_two_log" 2>&1 &
peer_pid=$!

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

for completed_report in "$report_path" "$peer_report_path"; do
    if [[ ! -f "$completed_report" ]] || \
        ! awk -F= '$1 == "result" && $2 == "pass" { passed = 1 } END { exit !passed }' \
        "$completed_report"; then
        printf 'brawler render evidence: report failed its locked threshold: %s\n' \
            "$completed_report" >&2
        exit 1
    fi
done
printf 'brawler render evidence: passed; reports=%s,%s\n' "$report_path" "$peer_report_path"
