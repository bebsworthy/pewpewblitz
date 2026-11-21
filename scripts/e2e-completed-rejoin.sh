#!/usr/bin/env bash
set -euo pipefail

# BRL-jxfy: a real supervisor, lobby and match worker, and two headless clients in a PvP 1v1
# match. Client 1 drops all traffic for longer than the 10 s routed session bound, so its seat is
# held, and then cues client 2, which sends Leave Match: the lone team forfeits and the match
# completes while client 1's seat is held. Client 1 comes back; the run passes only if it rejoins
# the completed match, captures the forfeit result with its own team as the winner, and returns
# to a fresh lobby.

game_type=${1:-takedown-1v1}
case "$game_type" in
    takedown-1v1) ;;
    *)
        printf '%s\n' 'brawler completed rejoin e2e: only takedown-1v1 is rehearsed' >&2
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

printf 'brawler completed rejoin e2e: running %s on %s\n' "$game_type" "$BRAWLER_ROUTED_BIND"
BRAWLER_NETWORK_HEADLESS=1 \
BRAWLER_PRODUCT_PLAYERS_PER_TEAM=1 \
BRAWLER_PRODUCT_CLIENT_COUNT=2 \
BRAWLER_PRODUCT_GAME_TYPE="$game_type" \
BRAWLER_PRODUCT_REJOIN_SMOKE=completed \
BRAWLER_ROUTED_TIMEOUT_SECONDS=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-120} \
exec ./scripts/network-product-match.sh
