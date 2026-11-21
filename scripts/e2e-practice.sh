#!/usr/bin/env bash
set -euo pipefail

game_type=${1:-takedown-1v1}
# Optional built-in map preset ID; Practice otherwise starts the game type's first map.
map_preset=${2:-}
case "$game_type" in
    takedown-1v1 | takedown-2v2 | takedown-3v3 | \
    turf-hold-1v1 | turf-hold-2v2 | turf-hold-3v3 | \
    vault-break-1v1 | vault-break-2v2 | vault-break-3v3 | \
    shard-rush-3v3 | \
    zombie-2p | zombie-3p) ;;
    *)
        printf '%s\n' 'brawler Practice e2e: expected an advertised game-type ID' >&2
        exit 2
        ;;
esac

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
    export BRAWLER_ROUTED_BIND="127.0.0.1:$port"
fi

printf 'brawler Practice e2e: running %s%s on %s\n' "$game_type" "${map_preset:+ map $map_preset}" "$BRAWLER_ROUTED_BIND"
BRAWLER_PRODUCT_PRACTICE_MAP="$map_preset" \
BRAWLER_NETWORK_HEADLESS=1 \
BRAWLER_PRODUCT_CLIENT_COUNT=1 \
BRAWLER_PRODUCT_PRACTICE_SMOKE=1 \
BRAWLER_PRODUCT_GAME_TYPE="$game_type" \
exec ./scripts/network-product-match.sh
