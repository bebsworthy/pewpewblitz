#!/bin/sh
set -eu

# Product path: form exact 1v1, 2v2, 3v3, or two/three-player Zombie reservations, deliver every
# participant's grant, connect each fresh match session, check in, and exit after authoritative
# Active. M06 may override the client count to exercise several serial handoffs from one queue pool.

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$project_dir/scripts/e2e-config.sh"
bind_addr=${BRAWLER_ROUTED_BIND:-127.0.0.1:5000}
# Keep automated runs out of the supervisor's default data/server/ profile store.
server_data_dir=${BRAWLER_SERVER_DATA_DIR:-$project_dir/target/e2e/server}
timeout_seconds=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-}
players_per_team=${BRAWLER_PRODUCT_PLAYERS_PER_TEAM:-2}
game_type=${BRAWLER_PRODUCT_GAME_TYPE:-}
headless=${BRAWLER_NETWORK_HEADLESS:-1}
requeue_smoke=${BRAWLER_PRODUCT_REQUEUE_SMOKE:-0}
practice_smoke=${BRAWLER_PRODUCT_PRACTICE_SMOKE:-0}
network_change_smoke=${BRAWLER_PRODUCT_NETWORK_CHANGE_SMOKE:-0}
rejoin_smoke=${BRAWLER_PRODUCT_REJOIN_SMOKE:-0}
# Optional built-in map preset ID for the Practice smoke (BRL-9gqj); defaults to the first map.
practice_map=${BRAWLER_PRODUCT_PRACTICE_MAP:-}
case "$headless" in
    0 | 1) ;;
    *)
        echo "brawler product match: BRAWLER_NETWORK_HEADLESS must be 0 or 1" >&2
        exit 2
        ;;
esac
case "$requeue_smoke" in
    0 | 1) ;;
    *)
        echo "brawler product match: BRAWLER_PRODUCT_REQUEUE_SMOKE must be 0 or 1" >&2
        exit 2
        ;;
esac
case "$practice_smoke" in
    0 | 1) ;;
    *)
        echo "brawler product match: BRAWLER_PRODUCT_PRACTICE_SMOKE must be 0 or 1" >&2
        exit 2
        ;;
esac
case "$network_change_smoke" in
    0) practice_flag=--product-practice-smoke ;;
    1)
        [ "$practice_smoke" = 1 ] || {
            echo "brawler product match: BRAWLER_PRODUCT_NETWORK_CHANGE_SMOKE requires Practice smoke" >&2
            exit 2
        }
        practice_flag=--product-network-change-smoke
        ;;
    *)
        echo "brawler product match: BRAWLER_PRODUCT_NETWORK_CHANGE_SMOKE must be 0 or 1" >&2
        exit 2
        ;;
esac
case "$rejoin_smoke" in
    0) ;;
    1)
        { [ "$practice_smoke" = 1 ] && [ "$network_change_smoke" = 0 ]; } || {
            echo "brawler product match: BRAWLER_PRODUCT_REJOIN_SMOKE requires Practice smoke without the network-change smoke" >&2
            exit 2
        }
        practice_flag=--product-rejoin-smoke
        ;;
    completed)
        # BRL-jxfy: client 1 loses its session beyond the session bound and cues client 2 to
        # leave; the lone team forfeits while client 1's seat is held, and client 1 rejoins the
        # completed match for its Results.
        { [ "$practice_smoke" = 0 ] && [ "$requeue_smoke" = 0 ] && [ "$players_per_team" = 1 ]; } || {
            echo "brawler product match: BRAWLER_PRODUCT_REJOIN_SMOKE=completed requires one PvP 1v1 roster" >&2
            exit 2
        }
        ;;
    *)
        echo "brawler product match: BRAWLER_PRODUCT_REJOIN_SMOKE must be 0, 1 or completed" >&2
        exit 2
        ;;
esac
case "$practice_map" in
    '') ;;
    *[!0-9]* | 0*)
        echo "brawler product match: BRAWLER_PRODUCT_PRACTICE_MAP must be a built-in map preset ID" >&2
        exit 2
        ;;
    *)
        [ "$practice_smoke" = 1 ] || {
            echo "brawler product match: BRAWLER_PRODUCT_PRACTICE_MAP requires Practice smoke" >&2
            exit 2
        }
        ;;
esac
product_topology=pvp
case "$game_type" in
    zombie-2p)
        product_topology=zombie
        players_per_team=2
        roster_size=2
        match_flag=--product-match-smoke
        ;;
    zombie-3p)
        product_topology=zombie
        players_per_team=3
        roster_size=3
        match_flag=--product-match-smoke-3v3
        ;;
    *)
        case "$players_per_team" in
            1) roster_size=2; match_flag=--product-match-smoke-1v1 ;;
            2) roster_size=4; match_flag=--product-match-smoke ;;
            3) roster_size=6; match_flag=--product-match-smoke-3v3 ;;
            *)
                echo "brawler product match: players per team must be 1, 2, or 3" >&2
                exit 2
                ;;
        esac
        ;;
esac
if [ "$practice_smoke" = 1 ]; then
    if [ -z "$game_type" ]; then
        echo "brawler product match: Practice smoke requires BRAWLER_PRODUCT_GAME_TYPE" >&2
        exit 2
    fi
    client_count=${BRAWLER_PRODUCT_CLIENT_COUNT:-1}
else
    client_count=${BRAWLER_PRODUCT_CLIENT_COUNT:-$roster_size}
fi
case "$client_count" in
    1) [ "$practice_smoke" = 1 ] || {
        echo "brawler product match: one client is reserved for Practice smoke" >&2
        exit 2
    } ;;
    2 | 4 | 6) ;;
    3) [ "$product_topology" = zombie ] || {
        echo "brawler product match: three clients are reserved for Zombie 3-player co-op" >&2
        exit 2
    } ;;
    *)
        echo "brawler product match: client count must be 1 for Practice, 2/4/6 for PvP, or 2/3 for Zombie" >&2
        exit 2
        ;;
esac
if [ "$practice_smoke" = 0 ] && [ $((client_count % roster_size)) -ne 0 ]; then
    echo "brawler product match: client count must contain whole ${players_per_team}v${players_per_team} rosters" >&2
    exit 2
fi
if [ "$requeue_smoke" = 1 ] && { [ "$players_per_team" -ne 1 ] || [ "$client_count" -ne 2 ]; }; then
    echo "brawler product match: requeue smoke requires exactly one 1v1 roster" >&2
    exit 2
fi
if [ "$rejoin_smoke" = completed ] && [ "$client_count" -ne 2 ]; then
    echo "brawler product match: the completed rejoin smoke requires exactly one 1v1 roster" >&2
    exit 2
fi
# The requeue smoke plays a real match to its authoritative end, so its default watchdog follows
# the game type's authored timing instead of the fast Active-only bound (BRL-cb3x).
if [ "$requeue_smoke" = 1 ]; then
    game_type=${game_type:-takedown-1v1}
    if [ -z "$timeout_seconds" ]; then
        timeout_seconds=$(e2e_requeue_timeout_seconds "$project_dir" "$game_type") || {
            echo "brawler product match: config/server/game-types.ron has no complete timing for $game_type" >&2
            exit 2
        }
        echo "brawler product match: requeue watchdog ${timeout_seconds}s covers $game_type formation, countdown, and match duration plus ${e2e_requeue_margin_seconds}s routing margin"
    fi
fi
timeout_seconds=${timeout_seconds:-60}
# Characters come from the authored diagnostic rotation, never from a literal range (BRL-xbq8).
first_character=$(e2e_diagnostic_character_for_client "$project_dir" 1) || {
    echo "brawler product match: content/catalogs/builds.ron has no direct_diagnostic picks" >&2
    exit 2
}

supervisor_pid=
watchdog_pid=
client_1= client_2= client_3= client_4= client_5= client_6=
cue_dir=
requeue_log_dir=

# Name the furthest requeue lifecycle phase each client and the supervisor reached, so a failure
# reads as "never completed", "never returned to lobby", or "no new Joined".
report_requeue_phases() {
    [ -n "$requeue_log_dir" ] || return 0
    phase_index=1
    while [ "$phase_index" -le "$client_count" ]; do
        phase_log="$requeue_log_dir/client-$phase_index.log"
        phase=$(awk '
            /brawler client accepted/ { granted = 1 }
            /observed authoritative match completion/ { completed = 1 }
            completed && /brawler lobby client accepted/ { lobby = 1 }
            /requeue smoke accepted a fresh queue Join/ { joined = 1 }
            END {
                if (joined) print "received a new queue Joined"
                else if (lobby) print "returned to a fresh lobby but got no new queue Joined"
                else if (completed) print "saw the match complete but never returned to a fresh lobby"
                else if (granted) print "entered the match but never saw it complete"
                else print "never entered a match"
            }
        ' "$phase_log" 2>/dev/null || echo "left no log")
        echo "brawler product match: requeue phase: client $phase_index $phase ($phase_log)" >&2
        phase_index=$((phase_index + 1))
    done
    results=$(grep -c "worker result-received" "$requeue_log_dir/supervisor.log" 2>/dev/null || true)
    reaped=$(grep -c "worker reaped" "$requeue_log_dir/supervisor.log" 2>/dev/null || true)
    echo "brawler product match: requeue phase: supervisor received ${results:-0} match result(s) and reaped ${reaped:-0} worker(s)" >&2
}

cleanup() {
    trap - INT TERM EXIT
    if [ -n "$cue_dir" ]; then
        rm -rf "$cue_dir"
    fi
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
cargo build --locked -p brawler-client --bin brawler-client --features automation
cargo build --locked --bin brawler-server --no-default-features --features server
cargo build --locked -p brawler-routing --bin brawler-supervisor

identity=$(target/debug/brawler-server routing-identity)
network_protocol=$(printf '%s\n' "$identity" | awk -F= '$1 == "network_protocol" { print $2 }')
registry_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "protocol_registry_fingerprint" { print $2 }')
content_fingerprint=$(printf '%s\n' "$identity" | awk -F= '$1 == "content_fingerprint" { print $2 }')

# The requeue smoke keeps each process's log as its phase evidence; other paths stream to stderr.
if [ "$requeue_smoke" = 1 ]; then
    mkdir -p "$project_dir/target/e2e"
    requeue_log_dir=$(mktemp -d "$project_dir/target/e2e/requeue.XXXXXX")
    echo "brawler product match: requeue logs in $requeue_log_dir"
    exec 3>"$requeue_log_dir/supervisor.log"
else
    exec 3>&2
fi
target/debug/brawler-supervisor \
    --network-protocol "$network_protocol" \
    --protocol-registry-fingerprint "$registry_fingerprint" \
    --content-fingerprint "$content_fingerprint" \
    --worker-executable "$project_dir/target/debug/brawler-server" \
    --game-types "$project_dir/config/server/game-types.ron" \
    --data-directory "$server_data_dir" \
    --bind "$bind_addr" 2>&3 &
supervisor_pid=$!
exec 3>&-

index=1
while [ "$index" -le "$client_count" ]; do
    character=$(e2e_diagnostic_character_for_client "$project_dir" "$index")
    if [ "$headless" = 1 ]; then
        if [ "$practice_smoke" = 1 ]; then
            target/debug/brawler-client \
                --client-id $((5000 + index)) \
                --server "$bind_addr" \
                --transport routed-udp \
                --auto-connect \
                --headless \
                "$practice_flag" \
                --product-game-type "$game_type" \
                ${practice_map:+--product-practice-map "$practice_map"} \
                --character "$first_character" &
        elif [ "$rejoin_smoke" = completed ]; then
            if [ "$index" -eq 1 ]; then
                role_flag=--product-completed-rejoin-smoke
            else
                role_flag=--leave-match-cue
            fi
            if [ -z "$cue_dir" ]; then
                mkdir -p "$project_dir/target/e2e"
                cue_dir=$(mktemp -d "$project_dir/target/e2e/completed-rejoin.XXXXXX")
            fi
            if [ "$index" -eq 1 ]; then
                target/debug/brawler-client \
                    --client-id $((5000 + index)) \
                    --server "$bind_addr" \
                    --transport routed-udp \
                    --auto-connect \
                    --headless \
                    "$match_flag" \
                    --product-game-type "${game_type:-takedown-1v1}" \
                    "$role_flag" "$cue_dir/leave" \
                    --character "$first_character" &
            else
                target/debug/brawler-client \
                    --client-id $((5000 + index)) \
                    --server "$bind_addr" \
                    --transport routed-udp \
                    --auto-connect \
                    --headless \
                    "$match_flag" \
                    --product-game-type "${game_type:-takedown-1v1}" \
                    "$role_flag" "$cue_dir/leave" \
                    --exit-after-lobby-return \
                    --character "$first_character" &
            fi
        elif [ "$requeue_smoke" = 1 ]; then
            if [ "$index" -eq 1 ]; then aim_axis=1,0; else aim_axis=-1,0; fi
            target/debug/brawler-client \
                --client-id $((5000 + index)) \
                --server "$bind_addr" \
                --transport routed-udp \
                --auto-connect \
                --headless \
                --product-requeue-smoke \
                --product-game-type "$game_type" \
                --aim-axis "$aim_axis" \
                --fire \
                --simulation-ticks 4000 \
                --character "$first_character" >"$requeue_log_dir/client-$index.log" 2>&1 &
        else
            if [ -n "$game_type" ]; then
                target/debug/brawler-client \
                    --client-id $((5000 + index)) \
                    --server "$bind_addr" \
                    --transport routed-udp \
                    --auto-connect \
                    --headless \
                    "$match_flag" \
                    --product-game-type "$game_type" \
                    --character "$character" &
            else
                target/debug/brawler-client \
                    --client-id $((5000 + index)) \
                    --server "$bind_addr" \
                    --transport routed-udp \
                    --auto-connect \
                    --headless \
                    "$match_flag" \
                    --character "$character" &
            fi
        fi
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
        report_requeue_phases
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
    report_requeue_phases
    exit "$status"
fi

report_requeue_phases
if [ -n "$requeue_log_dir" ] && ! grep -q "worker result-received" "$requeue_log_dir/supervisor.log"; then
    echo "brawler product match: the supervisor never received the completed match result" >&2
    exit 1
fi
kill -INT "$supervisor_pid"
wait "$supervisor_pid"
supervisor_pid=
if [ "$headless" = 1 ]; then
    roster_count=$((client_count / roster_size))
    if [ "$rejoin_smoke" = completed ]; then
        echo "brawler product match: ${game_type:-takedown-1v1} completed by forfeit while a seat was held, and that player rejoined it for its Results"
    elif [ "$rejoin_smoke" = 1 ]; then
        echo "brawler product match: Practice $game_type rejoined the same running match after an outage beyond the session bound"
    elif [ "$network_change_smoke" = 1 ]; then
        echo "brawler product match: Practice $game_type kept its match session across a source-port change and a return to the original port"
    elif [ "$practice_smoke" = 1 ]; then
        echo "brawler product match: Practice $game_type${practice_map:+ (map preset $practice_map)} reached Active with one human"
    elif [ "$requeue_smoke" = 1 ]; then
        echo "brawler product match: Results -> Queue Again received a fresh Joined outcome"
    else
        if [ "$product_topology" = zombie ]; then
            echo "brawler product match: ${roster_count} exact Zombie ${players_per_team}-player cooperative roster(s) reached Active"
        else
            echo "brawler product match: ${roster_count} exact ${players_per_team}v${players_per_team} roster(s) reached Active"
        fi
    fi
else
    if [ "$product_topology" = zombie ]; then
        echo "brawler product match: windowed Zombie ${players_per_team}-player cooperative session closed cleanly"
    else
        echo "brawler product match: windowed ${players_per_team}v${players_per_team} session closed cleanly"
    fi
fi
