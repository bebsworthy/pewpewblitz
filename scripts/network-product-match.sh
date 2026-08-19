#!/bin/sh
set -eu

# M05 product path: form one exact 1v1, 2v2, or 3v3 reservation, deliver each participant's grant,
# connect every fresh match session, check in, and exit after authoritative Active.

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bind_addr=${BRAWLER_ROUTED_BIND:-127.0.0.1:5000}
timeout_seconds=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-60}
players_per_team=${BRAWLER_PRODUCT_PLAYERS_PER_TEAM:-2}
headless=${BRAWLER_NETWORK_HEADLESS:-1}
case "$headless" in
    0 | 1) ;;
    *)
        echo "brawler product match: BRAWLER_NETWORK_HEADLESS must be 0 or 1" >&2
        exit 2
        ;;
esac
case "$players_per_team" in
    1) client_count=2; match_flag=--product-match-smoke-1v1 ;;
    2) client_count=4; match_flag=--product-match-smoke ;;
    3) client_count=6; match_flag=--product-match-smoke-3v3 ;;
    *)
        echo "brawler product match: players per team must be 1, 2, or 3" >&2
        exit 2
        ;;
esac

supervisor_pid=
watchdog_pid=
client_1= client_2= client_3= client_4= client_5= client_6=

cleanup() {
    trap - INT TERM EXIT
    for pid in "$watchdog_pid" "$client_1" "$client_2" "$client_3" "$client_4" "$client_5" "$client_6"; do
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    if [ -n "$supervisor_pid" ] && kill -0 "$supervisor_pid" 2>/dev/null; then
        kill -INT "$supervisor_pid" 2>/dev/null || true
    fi
    for pid in "$watchdog_pid" "$client_1" "$client_2" "$client_3" "$client_4" "$client_5" "$client_6" "$supervisor_pid"; do
        if [ -n "$pid" ]; then
            wait "$pid" 2>/dev/null || true
        fi
    done
}

abort() {
    cleanup
    exit 124
}

trap abort INT TERM
trap cleanup EXIT

cd "$project_dir"
cargo build --locked --bin brawler-client --no-default-features --features client
cargo build --locked --bin brawler-server --no-default-features --features server
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$(target/debug/brawler-server routing-identity)
network_protocol=$(printf '%s\n' "$identity" | awk -F= '$1 == "network_protocol" { print $2 }')
registry_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }')
content_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "content_fingerprint" { print $2 }')

target/debug/brawler-supervisor \
    --network-protocol "$network_protocol" \
    --protocol-registry-fingerprint "$registry_fingerprint" \
    --content-fingerprint "$content_fingerprint" \
    --worker-executable "$project_dir/target/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --bind "$bind_addr" &
supervisor_pid=$!

index=1
while [ "$index" -le "$client_count" ]; do
    preset=$((1 + (index - 1) % 5))
    if [ "$headless" = 1 ]; then
        target/debug/brawler-client \
            --client-id $((5000 + index)) \
            --server "$bind_addr" \
            --transport routed-udp \
            --auto-connect \
            --headless \
            "$match_flag" \
            --build-preset "$preset" &
    else
        target/debug/brawler-client \
            --server "$bind_addr" \
            --transport routed-udp &
    fi
    pid=$!
    eval "client_${index}=\$pid"
    # Avoid turning a local multi-process smoke into a burst test of the deliberately small
    # unauthenticated lobby ingress budget. Network impairment coverage owns burst behavior.
    sleep 2
    index=$((index + 1))
done

if [ "$headless" = 0 ]; then
    echo "brawler product match: use Play in each window, then join the advertised ${players_per_team}v${players_per_team} game"
fi

if [ "$headless" = 1 ]; then
    parent_pid=$$
    (
        sleep "$timeout_seconds"
        echo "brawler product match: timed out after ${timeout_seconds}s" >&2
        index=1
        while [ "$index" -le "$client_count" ]; do
            eval "pid=\$client_${index}"
            kill -TERM "$pid" 2>/dev/null || true
            index=$((index + 1))
        done
        kill -TERM "$supervisor_pid" 2>/dev/null || true
        kill -TERM "$parent_pid"
    ) &
    watchdog_pid=$!
fi

status=0
index=1
while [ "$index" -le "$client_count" ]; do
    eval "pid=\$client_${index}"
    wait "$pid" || status=$?
    eval "client_${index}="
    index=$((index + 1))
done
kill "$watchdog_pid" 2>/dev/null || true
wait "$watchdog_pid" 2>/dev/null || true
watchdog_pid=
if [ "$status" -ne 0 ]; then
    echo "brawler product match: a client failed with status $status" >&2
    exit "$status"
fi

kill -INT "$supervisor_pid"
wait "$supervisor_pid"
supervisor_pid=
if [ "$headless" = 1 ]; then
    echo "brawler product match: exact ${players_per_team}v${players_per_team} reached Active"
else
    echo "brawler product match: windowed ${players_per_team}v${players_per_team} session closed cleanly"
fi
