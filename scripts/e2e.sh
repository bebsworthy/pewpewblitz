#!/usr/bin/env bash
set -euo pipefail

client_count=${1:-2}
case "$client_count" in
    2) players_per_team=1 ;;
    4) players_per_team=2 ;;
    6) players_per_team=3 ;;
    *)
        printf '%s\n' 'brawler e2e: client count must be 2, 4, or 6' >&2
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

printf 'brawler e2e: running %s clients on %s\n' "$client_count" "$BRAWLER_ROUTED_BIND"

BRAWLER_NETWORK_HEADLESS=1 \
BRAWLER_PRODUCT_PLAYERS_PER_TEAM="$players_per_team" \
exec ./scripts/network-product-match.sh
