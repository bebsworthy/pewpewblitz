#!/usr/bin/env python3
"""Verify the routed public-UDP MTU/fragmentation evidence in a classic pcap.

This intentionally uses only the Python standard library so the optional macOS capture gate does
not add a project dependency.  macOS ``tcpdump -w`` emits the classic pcap format by default.
The verifier accepts the loopback (DLT_NULL), Ethernet, raw-IP, and Linux cooked link types that
are useful when the same capture is inspected on a development host.

The command is evidence-bearing only when it sees at least one IPv4 or IPv6 UDP datagram.  An
empty, truncated, unsupported, or malformed capture exits non-zero and must remain ``unsupported``
in the M01 report; this tool never fabricates a passing capture result.
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path
from typing import Any, Iterator


PCAP_MAGIC = {
    b"\xd4\xc3\xb2\xa1": "<",  # microsecond timestamps, little endian
    b"\xa1\xb2\xc3\xd4": ">",  # microsecond timestamps, big endian
    b"\x4d\x3c\xb2\xa1": "<",  # nanosecond timestamps, little endian
    b"\xa1\xb2\x3c\x4d": ">",  # nanosecond timestamps, big endian
}

DLT_NULL = 0
DLT_EN10MB = 1
DLT_RAW = 12
DLT_LINUX_SLL = 113
DLT_LINUX_SLL2 = 276

ETHERTYPE_IPV4 = 0x0800
ETHERTYPE_IPV6 = 0x86DD
ETHERTYPE_VLAN = {0x8100, 0x88A8, 0x9100}

IPPROTO_HOPOPT = 0
IPPROTO_TCP = 6
IPPROTO_UDP = 17
IPPROTO_ROUTING = 43
IPPROTO_FRAGMENT = 44
IPPROTO_ESP = 50
IPPROTO_AH = 51
IPPROTO_NONE = 59
IPPROTO_DSTOPTS = 60

UDP_PAYLOAD_LIMIT = 1_200


class CaptureError(Exception):
    """The capture cannot provide authoritative evidence."""


def _read_exact(data: bytes, start: int, size: int, what: str) -> bytes:
    end = start + size
    if end > len(data):
        raise CaptureError(f"truncated {what}")
    return data[start:end]


def _link_payload(link_type: int, packet: bytes, endian: str) -> tuple[int, bytes] | None:
    """Return ``(ethertype, network_packet)`` for supported link headers."""

    if link_type == DLT_NULL:
        family_bytes = _read_exact(packet, 0, 4, "DLT_NULL family")
        # DLT_NULL stores the address family in host order. A capture may be moved between hosts,
        # so accept either interpretation while still requiring an IPv4/IPv6 value.
        family = int.from_bytes(family_bytes, byteorder="little" if endian == "<" else "big")
        if family not in (2, 24, 28, 30):
            swapped = int.from_bytes(
                family_bytes, byteorder="big" if endian == "<" else "little"
            )
            family = swapped
        if family == 2:
            return ETHERTYPE_IPV4, packet[4:]
        if family in (24, 28, 30):
            return ETHERTYPE_IPV6, packet[4:]
        return None
    if link_type == DLT_EN10MB:
        ethertype = int.from_bytes(_read_exact(packet, 12, 2, "Ethernet ethertype"), "big")
        offset = 14
        while ethertype in ETHERTYPE_VLAN:
            ethertype = int.from_bytes(_read_exact(packet, offset + 2, 2, "VLAN ethertype"), "big")
            offset += 4
        return ethertype, packet[offset:]
    if link_type == DLT_RAW:
        first = _read_exact(packet, 0, 1, "raw IP version")[0] >> 4
        if first == 4:
            return ETHERTYPE_IPV4, packet
        if first == 6:
            return ETHERTYPE_IPV6, packet
        return None
    if link_type == DLT_LINUX_SLL:
        protocol = int.from_bytes(_read_exact(packet, 14, 2, "Linux cooked protocol"), "big")
        return protocol, packet[16:]
    if link_type == DLT_LINUX_SLL2:
        protocol = int.from_bytes(_read_exact(packet, 0, 2, "Linux cooked v2 protocol"), "big")
        return protocol, packet[20:]
    raise CaptureError(f"unsupported pcap link type DLT_{link_type}")


def _udp_payload_length(packet: bytes, ethertype: int) -> tuple[int, bool, bool] | None:
    """Return ``(payload_length, ipv4_fragmented, ipv6_fragment_header)`` for UDP."""

    if ethertype == ETHERTYPE_IPV4:
        first = _read_exact(packet, 0, 1, "IPv4 header")[0]
        if first >> 4 != 4:
            raise CaptureError("IPv4 link payload has a non-IPv4 version")
        ihl = (first & 0x0F) * 4
        if ihl < 20:
            raise CaptureError("IPv4 IHL is below the minimum")
        header = _read_exact(packet, 0, ihl, "IPv4 header")
        total_length = int.from_bytes(header[2:4], "big")
        if total_length < ihl:
            raise CaptureError("IPv4 total length is below the header length")
        flags_offset = int.from_bytes(header[6:8], "big")
        fragmented = bool(flags_offset & 0x2000 or flags_offset & 0x1FFF)
        if header[9] != IPPROTO_UDP:
            return None
        udp = _read_exact(packet, ihl, 8, "UDP header")
        udp_length = int.from_bytes(udp[4:6], "big")
        if udp_length < 8:
            raise CaptureError("UDP length is below the 8-byte header")
        if ihl + udp_length > total_length:
            raise CaptureError("UDP length exceeds the IPv4 packet")
        return udp_length - 8, fragmented, False

    if ethertype != ETHERTYPE_IPV6:
        return None
    header = _read_exact(packet, 0, 40, "IPv6 header")
    if header[0] >> 4 != 6:
        raise CaptureError("IPv6 link payload has a non-IPv6 version")
    payload_length = int.from_bytes(header[4:6], "big")
    next_header = header[6]
    offset = 40
    fragment_header = False
    # Walk the bounded extension chain to locate UDP. Fragment is recorded even when it is not
    # possible to inspect the following UDP header (a non-zero fragment offset has no UDP header).
    for _ in range(16):
        if next_header == IPPROTO_UDP:
            udp = _read_exact(packet, offset, 8, "UDP header")
            udp_length = int.from_bytes(udp[4:6], "big")
            if udp_length < 8:
                raise CaptureError("UDP length is below the 8-byte header")
            if offset + udp_length > 40 + payload_length:
                raise CaptureError("UDP length exceeds the IPv6 packet")
            return udp_length - 8, False, fragment_header
        if next_header == IPPROTO_FRAGMENT:
            fragment_header = True
            extension = _read_exact(packet, offset, 8, "IPv6 Fragment header")
            next_header = extension[0]
            offset += 8
            continue
        if next_header in (IPPROTO_HOPOPT, IPPROTO_ROUTING, IPPROTO_DSTOPTS):
            extension = _read_exact(packet, offset, 2, "IPv6 extension header")
            length = (extension[1] + 1) * 8
        elif next_header == IPPROTO_AH:
            extension = _read_exact(packet, offset, 2, "IPv6 AH header")
            length = (extension[1] + 2) * 4
        else:
            # ESP and unknown extension payloads are opaque to this verifier. It is safer to
            # ignore the packet than to claim that an unseen UDP payload was below the limit.
            return None
        if length < 8 or offset + length > 40 + payload_length:
            raise CaptureError("invalid IPv6 extension-header length")
        next_header = packet[offset]
        offset += length
    raise CaptureError("IPv6 extension-header chain exceeds bounded parser depth")


def _records(data: bytes) -> tuple[int, str, Iterator[bytes]]:
    if len(data) < 24:
        raise CaptureError("pcap global header is truncated")
    magic = data[:4]
    endian = PCAP_MAGIC.get(magic)
    if endian is None:
        if magic == b"\x0a\x0d\x0d\x0a":
            raise CaptureError("pcapng is unsupported; capture with macOS tcpdump -w classic pcap")
        raise CaptureError("unrecognized pcap magic")
    version_major, version_minor, _zone, _sigfigs, snaplen, link_type = struct.unpack(
        f"{endian}HHiiii", data[4:24]
    )
    if (version_major, version_minor) != (2, 4):
        raise CaptureError(f"unsupported pcap version {version_major}.{version_minor}")
    if snaplen == 0:
        raise CaptureError("pcap snaplen is zero")

    def iterator() -> Iterator[bytes]:
        offset = 24
        while offset < len(data):
            if offset + 16 > len(data):
                raise CaptureError("truncated pcap packet header")
            _seconds, _fraction, captured, original = struct.unpack(
                f"{endian}IIII", data[offset : offset + 16]
            )
            offset += 16
            if captured > snaplen or captured > original:
                raise CaptureError("invalid pcap captured/original packet lengths")
            packet = _read_exact(data, offset, captured, "pcap packet data")
            offset += captured
            yield packet

    return link_type, endian, iterator()


def verify_capture(path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    link_type, endian, packets = _records(data)
    packet_count = 0
    ip_count = 0
    ipv4_count = 0
    ipv6_count = 0
    ipv4_udp_count = 0
    ipv6_udp_count = 0
    udp_count = 0
    max_udp_payload = 0
    fragmented_ipv4 = 0
    ipv6_fragments = 0
    violations: list[dict[str, Any]] = []
    for packet_index, packet in enumerate(packets):
        packet_count += 1
        try:
            link = _link_payload(link_type, packet, endian)
            if link is None:
                continue
            ethertype, ip_packet = link
            if ethertype not in (ETHERTYPE_IPV4, ETHERTYPE_IPV6):
                continue
            ip_count += 1
            if ethertype == ETHERTYPE_IPV4:
                ipv4_count += 1
            else:
                ipv6_count += 1
            parsed = _udp_payload_length(ip_packet, ethertype)
            if parsed is None:
                continue
            payload_length, ipv4_fragmented, ipv6_fragment = parsed
            udp_count += 1
            if ethertype == ETHERTYPE_IPV4:
                ipv4_udp_count += 1
            else:
                ipv6_udp_count += 1
            max_udp_payload = max(max_udp_payload, payload_length)
            if ipv4_fragmented:
                fragmented_ipv4 += 1
                violations.append({"packet": packet_index, "reason": "IPv4 fragmentation"})
            if ipv6_fragment:
                ipv6_fragments += 1
                violations.append({"packet": packet_index, "reason": "IPv6 Fragment header"})
            if payload_length > UDP_PAYLOAD_LIMIT:
                violations.append(
                    {
                        "packet": packet_index,
                        "reason": "UDP payload exceeds 1200 bytes",
                        "payload_bytes": payload_length,
                    }
                )
        except CaptureError as error:
            violations.append({"packet": packet_index, "reason": str(error)})

    status = (
        "passed"
        if ipv4_udp_count > 0 and ipv6_udp_count > 0 and not violations
        else "failed"
    )
    return {
        "schema": "brawler-routed-capture-v1",
        "capture": str(path),
        "link_type": link_type,
        "packet_count": packet_count,
        "ip_datagrams": ip_count,
        "ipv4_datagrams": ipv4_count,
        "ipv6_datagrams": ipv6_count,
        "udp_datagrams": udp_count,
        "ipv4_udp_datagrams": ipv4_udp_count,
        "ipv6_udp_datagrams": ipv6_udp_count,
        "max_udp_payload_bytes": max_udp_payload,
        "udp_payload_limit_bytes": UDP_PAYLOAD_LIMIT,
        "ipv4_fragmented_datagrams": fragmented_ipv4,
        "ipv6_fragment_headers": ipv6_fragments,
        "violations": violations,
        "required_families_observed": ipv4_udp_count > 0 and ipv6_udp_count > 0,
        "status": status,
        "evidence_status": "supported" if status == "passed" else "unsupported",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path, help="classic pcap produced by tcpdump -w")
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args(argv)
    try:
        result = verify_capture(args.capture)
    except (OSError, CaptureError) as error:
        result = {
            "schema": "brawler-routed-capture-v1",
            "capture": str(args.capture),
            "status": "unsupported",
            "evidence_status": "unsupported",
            "error": str(error),
        }
        if args.json:
            print(json.dumps(result, sort_keys=True))
        else:
            print(f"routed capture: unsupported ({error})", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(result, sort_keys=True))
    else:
        print(
            "routed capture: "
            f"status={result['status']} udp={result['udp_datagrams']} "
            f"max_payload={result['max_udp_payload_bytes']} "
            f"ipv4_fragments={result['ipv4_fragmented_datagrams']} "
            f"ipv6_fragment_headers={result['ipv6_fragment_headers']}"
        )
        for violation in result["violations"]:
            print(f"routed capture: violation {violation}", file=sys.stderr)
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
