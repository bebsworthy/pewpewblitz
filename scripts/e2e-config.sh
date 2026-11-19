# Sourced by the routed product smokes and `scripts/check-e2e-config.sh`; not executable.
#
# Reads the authored values those smokes depend on instead of repeating them as literals:
# diagnostic weapons from `content/catalogs/builds.ron` (BRL-xbq8) and game-type timing from
# `config/server/game-types.ron` (BRL-cb3x). Every reader fails closed: an empty answer means the
# authored file changed shape, and the caller must stop rather than guess.

# Seconds of routing and cleanup slack the requeue smoke allows beyond its authored match bound:
# worker result and reap, the fresh lobby session, and the second queue Join (BRL-cb3x).
e2e_requeue_margin_seconds=60

# The authored diagnostic weapon rotation, `direct_diagnostic.weapon_base_ids`, space-separated.
e2e_diagnostic_weapon_ids() {
    awk '
        /direct_diagnostic:/ { in_policy = 1 }
        in_policy && /weapon_base_ids:/ {
            line = $0
            while (match(line, /WeaponBaseId\([0-9]+\)/)) {
                printf "%s%s", separator, substr(line, RSTART + 13, RLENGTH - 14)
                separator = " "
                line = substr(line, RSTART + RLENGTH)
            }
            exit
        }
    ' "$1/content/catalogs/builds.ron"
}

# The costed (saved-brawler selectable) weapon IDs in `weapon_costs`, space-separated.
e2e_costed_weapon_ids() {
    awk '
        match($0, /weapon_id: WeaponPresetId\([0-9]+\)/) {
            printf "%s%s", separator, substr($0, RSTART + 26, RLENGTH - 27)
            separator = " "
        }
    ' "$1/content/catalogs/builds.ron"
}

# The `--weapon-preset` for one 1-based client index: the authored diagnostic rotation, wrapped.
e2e_diagnostic_weapon_for_client() {
    # shellcheck disable=SC2086 # Word splitting the space-separated ID list is intended.
    set -- "$2" $(e2e_diagnostic_weapon_ids "$1")
    e2e_client_index=$1
    shift
    [ "$#" -gt 0 ] || return 1
    shift $(((e2e_client_index - 1) % $#))
    printf '%s\n' "$1"
}

# One `name: <seconds>` integer field of the game-types file. With a game-type ID, the field is
# read from that game type's entry; without one, from the first line that carries it.
e2e_game_types_field() {
    awk -v id="$2" -v field="$3" '
        id == "" || index($0, "(id: \"" id "\",") {
            if (match($0, "[(, ]" field ": [0-9]+")) {
                print substr($0, RSTART + length(field) + 3, RLENGTH - length(field) - 3)
                exit
            }
        }
    ' "$1/config/server/game-types.ron"
}

# The requeue smoke's default watchdog for one game type: formation deadlines, countdown, the
# authoritative match duration, and the bounded routing margin.
e2e_requeue_timeout_seconds() {
    e2e_loading=$(e2e_game_types_field "$1" "" loading_deadline_seconds)
    e2e_grant=$(e2e_game_types_field "$1" "" grant_deadline_seconds)
    e2e_countdown=$(e2e_game_types_field "$1" "$2" countdown_seconds)
    e2e_duration=$(e2e_game_types_field "$1" "$2" match_duration_seconds)
    [ -n "$e2e_loading" ] && [ -n "$e2e_grant" ] && [ -n "$e2e_countdown" ] && [ -n "$e2e_duration" ] || return 1
    printf '%s\n' $((e2e_grant + e2e_loading + e2e_countdown + e2e_duration + e2e_requeue_margin_seconds))
}
