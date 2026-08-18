"""Focused unit tests for the classic-pcap routed MTU verifier."""

from __future__ import annotations

import importlib.util
import struct
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "verify_routed_capture", ROOT / "scripts" / "verify-routed-capture.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def udp(payload: bytes) -> bytes:
    return struct.pack("!HHHH", 50_000, 50_001, 8 + len(payload), 0) + payload


def ipv4(payload: bytes, flags_offset: int = 0) -> bytes:
    packet = struct.pack(
        "!BBHHHBBH4s4s",
        0x45,
        0,
        20 + len(udp(payload)),
        1,
        flags_offset,
        64,
        17,
        0,
        b"\x7f\x00\x00\x01",
        b"\x7f\x00\x00\x01",
    )
    return packet + udp(payload)


def ipv6(payload: bytes, fragment: bool = False) -> bytes:
    body = udp(payload)
    next_header = 44 if fragment else 17
    if fragment:
        body = struct.pack("!BBHI", 17, 0, 0, 0) + body
    header = struct.pack(
        "!IHBB16s16s",
        6 << 28,
        len(body),
        next_header,
        64,
        b"\x00" * 15 + b"\x01",
        b"\x00" * 15 + b"\x01",
    )
    return header + body


def ipv6_tcp(payload: bytes) -> bytes:
    header = struct.pack(
        "!IHBB16s16s",
        6 << 28,
        len(payload),
        6,
        64,
        b"\x00" * 15 + b"\x01",
        b"\x00" * 15 + b"\x01",
    )
    return header + payload


def pcap(packets: list[tuple[int, bytes]], endian: str = "<") -> bytes:
    result = bytearray(struct.pack(f"{endian}IHHiiii", 0xA1B2C3D4, 2, 4, 0, 0, 65_535, 0))
    for family, packet in packets:
        packet = struct.pack("<I", family) + packet
        if endian == ">":
            packet = struct.pack(">I", family) + packet[4:]
        result.extend(struct.pack(f"{endian}IIII", 0, 0, len(packet), len(packet)))
        result.extend(packet)
    return bytes(result)


class VerifyCaptureTests(unittest.TestCase):
    def run_capture(self, data: bytes) -> dict:
        with tempfile.NamedTemporaryFile(suffix=".pcap") as capture:
            capture.write(data)
            capture.flush()
            return MODULE.verify_capture(Path(capture.name))

    def test_ipv4_and_ipv6_exact_1200_boundary_is_passed(self) -> None:
        result = self.run_capture(
            pcap([(2, ipv4(b"a" * 1_200)), (30, ipv6(b"a" * 1_200))])
        )
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["max_udp_payload_bytes"], 1_200)
        self.assertEqual(result["ipv4_fragmented_datagrams"], 0)
        self.assertEqual(result["ipv6_fragment_headers"], 0)

    def test_both_address_families_are_required_and_big_endian_pcap_is_supported(self) -> None:
        ipv4_only = self.run_capture(pcap([(2, ipv4(b"a"))]))
        self.assertEqual(ipv4_only["status"], "failed")
        self.assertFalse(ipv4_only["required_families_observed"])
        mixed_l4 = self.run_capture(
            pcap([(2, ipv4(b"a")), (30, ipv6_tcp(b"not udp"))])
        )
        self.assertEqual(mixed_l4["status"], "failed")
        self.assertEqual(mixed_l4["ipv6_udp_datagrams"], 0)
        both_big_endian = self.run_capture(
            pcap([(2, ipv4(b"a")), (30, ipv6(b"a"))], endian=">")
        )
        self.assertEqual(both_big_endian["status"], "passed")

    def test_ipv4_fragment_and_ipv6_fragment_header_fail(self) -> None:
        result4 = self.run_capture(
            pcap([(2, ipv4(b"a", flags_offset=0x2000)), (30, ipv6(b"a"))])
        )
        result6 = self.run_capture(
            pcap([(2, ipv4(b"a")), (30, ipv6(b"a", fragment=True))])
        )
        self.assertEqual(result4["status"], "failed")
        self.assertEqual(result4["ipv4_fragmented_datagrams"], 1)
        self.assertEqual(result6["status"], "failed")
        self.assertEqual(result6["ipv6_fragment_headers"], 1)

    def test_payload_above_limit_fails(self) -> None:
        result = self.run_capture(
            pcap([(2, ipv4(b"a" * 1_201)), (30, ipv6(b"a"))])
        )
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["max_udp_payload_bytes"], 1_201)


if __name__ == "__main__":
    unittest.main()
