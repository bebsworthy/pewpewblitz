#!/usr/bin/env bash
set -euo pipefail

# BRL-0162: a real supervisor, lobby and match worker, and one headless client in a Practice
# match. Mid-match the client drops all traffic for 4 s, then resumes from a new source port; the
# run passes only if the same match session (no fresh lobby or match handoff) keeps receiving
# authoritative ticks afterwards.

game_type=${1:-wipeout-1v1}
export BRAWLER_PRODUCT_NETWORK_CHANGE_SMOKE=1
exec "$(dirname -- "$0")/e2e-practice.sh" "$game_type"
