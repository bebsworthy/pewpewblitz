#!/bin/sh
set -eu

# Regression check for the authored values the routed product smokes read (BRL-xbq8, BRL-cb3x):
# every client of the 2/4/6-client product paths gets a diagnostic weapon a saved brawler can
# select, and the requeue smoke's default watchdog covers its game type's authoritative match.

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$project_dir/scripts/e2e-config.sh"

fail() {
    echo "check-e2e-config: $*" >&2
    exit 1
}

diagnostic=$(e2e_diagnostic_weapon_ids "$project_dir")
costed=$(e2e_costed_weapon_ids "$project_dir")
[ -n "$diagnostic" ] || fail "no direct_diagnostic.weapon_base_ids in content/catalogs/builds.ron"
[ -n "$costed" ] || fail "no weapon_costs in content/catalogs/builds.ron"

for client_count in 2 4 6; do
    index=1
    while [ "$index" -le "$client_count" ]; do
        preset=$(e2e_diagnostic_weapon_for_client "$project_dir" "$index") ||
            fail "no diagnostic weapon for client $index"
        case " $diagnostic " in
            *" $preset "*) ;;
            *) fail "client $index of $client_count got $preset outside the diagnostic rotation ($diagnostic)" ;;
        esac
        case " $costed " in
            *" $preset "*) ;;
            *) fail "client $index of $client_count got $preset, which no saved brawler can select ($costed)" ;;
        esac
        index=$((index + 1))
    done
done

game_type=wipeout-1v1
timeout=$(e2e_requeue_timeout_seconds "$project_dir" "$game_type") ||
    fail "config/server/game-types.ron has no complete timing for $game_type"
duration=$(e2e_game_types_field "$project_dir" "$game_type" match_duration_seconds)
countdown=$(e2e_game_types_field "$project_dir" "$game_type" countdown_seconds)
[ "$timeout" -gt $((countdown + duration)) ] ||
    fail "requeue watchdog ${timeout}s does not cover the ${duration}s $game_type match"
if e2e_requeue_timeout_seconds "$project_dir" no-such-game-type >/dev/null; then
    fail "an unknown game type produced a requeue watchdog"
fi

echo "check-e2e-config: diagnostic weapons ($diagnostic) and the ${timeout}s $game_type requeue watchdog are authored"
