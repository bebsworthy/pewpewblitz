#!/usr/bin/env bash
set -euo pipefail

# BRL-g57a, BRL-sqz0: a real supervisor, lobby and match worker, and one headless client in a
# Practice match. Mid-match the client drops all traffic for 4 s and resumes from a new source
# port, then drops it for 4 s again and returns to its original source port (a Wi-Fi -> cellular
# -> Wi-Fi flap). The run passes only if the same match session (no fresh lobby or match handoff)
# keeps receiving authoritative ticks after each change.

game_type=${1:-wipeout-1v1}
export BRAWLER_PRODUCT_NETWORK_CHANGE_SMOKE=1
exec "$(dirname -- "$0")/e2e-practice.sh" "$game_type"
