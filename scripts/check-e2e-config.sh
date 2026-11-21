#!/bin/sh
set -eu

# Regression check for the authored values the routed product smokes read (BRL-xbq8, BRL-cb3x):
# every client of the 2/4/6-client product paths gets a diagnostic character from the roster, and the requeue smoke's default watchdog covers its game type's authoritative match.

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$project_dir/scripts/e2e-config.sh"

fail() {
    echo "check-e2e-config: $*" >&2
    exit 1
}

diagnostic=$(e2e_diagnostic_character_ids "$project_dir")
roster=$(e2e_roster_character_ids "$project_dir")
[ -n "$diagnostic" ] || fail "no direct_diagnostic.picks in content/catalogs/builds.ron"
[ -n "$roster" ] || fail "no characters in content/catalogs/characters.ron"

for client_count in 2 4 6; do
    index=1
    while [ "$index" -le "$client_count" ]; do
        character=$(e2e_diagnostic_character_for_client "$project_dir" "$index") ||
            fail "no diagnostic character for client $index"
        case " $diagnostic " in
            *" $character "*) ;;
            *) fail "client $index of $client_count got $character outside the diagnostic rotation ($diagnostic)" ;;
        esac
        case " $roster " in
            *" $character "*) ;;
            *) fail "client $index of $client_count got $character, which is not in the roster ($roster)" ;;
        esac
        index=$((index + 1))
    done
done

game_type=takedown-1v1
timeout=$(e2e_requeue_timeout_seconds "$project_dir" "$game_type") ||
    fail "config/server/game-types.ron has no complete timing for $game_type"
duration=$(e2e_game_types_field "$project_dir" "$game_type" match_duration_seconds)
countdown=$(e2e_game_types_field "$project_dir" "$game_type" countdown_seconds)
[ "$timeout" -gt $((countdown + duration)) ] ||
    fail "requeue watchdog ${timeout}s does not cover the ${duration}s $game_type match"
if e2e_requeue_timeout_seconds "$project_dir" no-such-game-type >/dev/null; then
    fail "an unknown game type produced a requeue watchdog"
fi

echo "check-e2e-config: diagnostic characters ($diagnostic) and the ${timeout}s $game_type requeue watchdog are authored"
