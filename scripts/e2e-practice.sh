#!/usr/bin/env bash
set -euo pipefail

game_type=${1:-wipeout-1v1}
case "$game_type" in
    wipeout-1v1 | wipeout-2v2 | wipeout-3v3 | \
    hot-zone-1v1 | hot-zone-2v2 | hot-zone-3v3 | \
    heist-1v1 | heist-2v2 | heist-3v3) ;;
    *)
        printf '%s\n' 'brawler Practice e2e: expected an advertised Feature Yard game-type ID' >&2
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

printf 'brawler Practice e2e: running %s on %s\n' "$game_type" "$BRAWLER_ROUTED_BIND"
BRAWLER_NETWORK_HEADLESS=1 \
BRAWLER_PRODUCT_CLIENT_COUNT=1 \
BRAWLER_PRODUCT_PRACTICE_SMOKE=1 \
BRAWLER_PRODUCT_GAME_TYPE="$game_type" \
exec ./scripts/network-product-match.sh
