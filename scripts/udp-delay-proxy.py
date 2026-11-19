#!/usr/bin/env python3
"""UDP proxy that delays server-to-client datagrams by a fixed time (BRL-jx6p).

Each client source gets its own upstream socket, so the server sees one distinct source per
client source, as it would without the proxy. Client-to-server datagrams pass through at once;
server-to-client datagrams are held for the delay, which adds that much to every round trip.
"""

import argparse
import heapq
import selectors
import signal
import socket
import sys
import time


def parse_address(value):
    host, _, port = value.rpartition(":")
    return host, int(port)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--listen", required=True, help="HOST:PORT clients send to")
    parser.add_argument("--upstream", required=True, help="HOST:PORT of the server")
    parser.add_argument("--delay-ms", type=int, required=True)
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
    delay = args.delay_ms / 1000.0
    upstream = parse_address(args.upstream)

    selector = selectors.DefaultSelector()
    public = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    public.bind(parse_address(args.listen))
    public.setblocking(False)
    selector.register(public, selectors.EVENT_READ, None)
    upstream_by_client = {}
    pending = []
    sequence = 0
    print(f"udp-delay-proxy listening={args.listen} upstream={args.upstream} delay_ms={args.delay_ms}",
          flush=True)

    while True:
        timeout = None
        if pending:
            timeout = max(0.0, pending[0][0] - time.monotonic())
        for key, _ in selector.select(timeout):
            sock = key.fileobj
            client = key.data
            while True:
                try:
                    payload, source = sock.recvfrom(65_535)
                except BlockingIOError:
                    break
                if client is None:
                    forward = upstream_by_client.get(source)
                    if forward is None:
                        forward = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                        forward.bind(("127.0.0.1", 0))
                        forward.setblocking(False)
                        selector.register(forward, selectors.EVENT_READ, source)
                        upstream_by_client[source] = forward
                    forward.sendto(payload, upstream)
                else:
                    sequence += 1
                    heapq.heappush(pending, (time.monotonic() + delay, sequence, client, payload))
        now = time.monotonic()
        while pending and pending[0][0] <= now:
            _, _, client, payload = heapq.heappop(pending)
            public.sendto(payload, client)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
