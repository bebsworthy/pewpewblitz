#!/bin/sh
set -eu

# Optional macOS-only MTU evidence. This wrapper deliberately does not invoke sudo: BPF capture
# permission is a host decision. If tcpdump cannot open the selected interface, the capture gate
# remains unsupported and the command returns a clear error instead of passing from a missing file.

if [ "$(uname -s)" != "Darwin" ]; then
    echo "brawler routed capture: unsupported outside macOS (use tcpdump manually and verify the pcap)" >&2
    exit 2
fi

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
capture_path=${1:-target/routed-capture.pcap}
interface=${BRAWLER_ROUTED_CAPTURE_INTERFACE:-lo0}
capture_filter=${BRAWLER_ROUTED_CAPTURE_FILTER:-udp and (port 5018 or port 5019)}
capture_pid=

cleanup() {
    trap - INT TERM EXIT
    if [ -n "$capture_pid" ] && kill -0 "$capture_pid" 2>/dev/null; then
        kill -INT "$capture_pid" 2>/dev/null || true
    fi
    if [ -n "$capture_pid" ]; then
        wait "$capture_pid" 2>/dev/null || true
    fi
}
trap cleanup INT TERM EXIT

cd "$project_dir"
mkdir -p "$(dirname -- "$capture_path")"
if [ -e "$capture_path" ]; then
    echo "brawler routed capture: refusing to overwrite existing capture ${capture_path}" >&2
    exit 2
fi

echo "brawler routed capture: starting tcpdump on ${interface}; BPF permission may require an approved macOS capture session" >&2
tcpdump -i "$interface" -n -s 0 -w "$capture_path" "$capture_filter" >/dev/null 2>"${capture_path}.tcpdump.log" &
capture_pid=$!

# Let tcpdump create and validate its BPF descriptor before the first client datagram. A fixed
# short wait is only process startup synchronization; the routed smoke itself remains bounded by
# its own watchdog and authority checks.
sleep 1
if ! kill -0 "$capture_pid" 2>/dev/null; then
    echo "brawler routed capture: tcpdump failed; inspect ${capture_path}.tcpdump.log (capture permission is required)" >&2
    exit 2
fi

BRAWLER_NETWORK_HEADLESS=1 BRAWLER_ROUTED_BIND=127.0.0.1:5018 ./scripts/network-routed.sh
BRAWLER_NETWORK_HEADLESS=1 BRAWLER_ROUTED_BIND='[::1]:5019' ./scripts/network-routed.sh

kill -INT "$capture_pid" 2>/dev/null || true
wait "$capture_pid" || true
capture_pid=

if [ ! -s "$capture_path" ]; then
    echo "brawler routed capture: tcpdump produced no capture; gate remains unsupported" >&2
    exit 2
fi
python3 scripts/verify-routed-capture.py --json "$capture_path"
