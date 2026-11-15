#!/usr/bin/env bash
set -euo pipefail

# BRL-yyeg: a real supervisor, lobby and match worker, and one headless client in a Practice
# match. Mid-match the client drops all traffic for longer than the 10 s routed session bound, so
# every party declares the match session lost. The client returns to a fresh lobby, which
# recognises the account's held seat and obtains a fresh capability, route and peer; the run
# passes only if the same player rejoins the same running match and receives authoritative ticks.

game_type=${1:-wipeout-1v1}
export BRAWLER_PRODUCT_REJOIN_SMOKE=1
export BRAWLER_ROUTED_TIMEOUT_SECONDS=${BRAWLER_ROUTED_TIMEOUT_SECONDS:-120}
exec "$(dirname -- "$0")/e2e-practice.sh" "$game_type"
