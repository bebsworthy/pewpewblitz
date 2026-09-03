#!/usr/bin/env bash
set -euo pipefail

player_count=${1:-2}
case "$player_count" in
    2) game_type=zombie-2p ;;
    3) game_type=zombie-3p ;;
    *)
        printf '%s\n' 'brawler Zombie e2e: player count must be 2 or 3' >&2
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

printf 'brawler Zombie e2e: running %s cooperative clients on %s\n' "$player_count" "$BRAWLER_ROUTED_BIND"

BRAWLER_NETWORK_HEADLESS=1 \
BRAWLER_PRODUCT_CLIENT_COUNT="$player_count" \
BRAWLER_PRODUCT_GAME_TYPE="$game_type" \
exec ./scripts/network-product-match.sh
