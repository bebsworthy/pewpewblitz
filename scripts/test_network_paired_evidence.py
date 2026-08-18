#!/usr/bin/env python3
"""Unit tests for the paired M01 measurement parser and gate math.

These tests never launch a process. Run with
``python3 -m unittest scripts/test_network_paired_evidence.py`` from the repository root.
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parent.parent
MODULE_PATH = ROOT / "scripts" / "network-paired-evidence.py"
SPEC = importlib.util.spec_from_file_location("network_paired_evidence", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PairedEvidenceTests(unittest.TestCase):
    def test_parse_cpu_time_supports_macos_and_linux_shapes(self) -> None:
        self.assertAlmostEqual(MODULE.parse_cpu_time("00:01.25"), 1.25)
        self.assertAlmostEqual(MODULE.parse_cpu_time("01:02:03.500"), 3723.5)
        self.assertAlmostEqual(MODULE.parse_cpu_time("1-02:03:04"), 93_784.0)
        self.assertIsNone(MODULE.parse_cpu_time("?"))
        self.assertIsNone(MODULE.parse_cpu_time("not-a-time"))

    def test_process_role_does_not_classify_build_commands(self) -> None:
        self.assertEqual(
            MODULE.process_role("target/debug/brawler-server --role lobby", "routed"),
            "lobby",
        )
        self.assertEqual(
            MODULE.process_role("target/debug/brawler-server --role=match", "routed"),
            "match",
        )
        self.assertEqual(
            MODULE.process_role("target/debug/brawler-server --bind 127.0.0.1:5000", "direct"),
            "server",
        )
        self.assertEqual(
            MODULE.process_role("target/debug/brawler-supervisor --bind 127.0.0.1:5000", "routed"),
            "supervisor",
        )
        self.assertEqual(
            MODULE.process_role("target/debug/brawler-client --client-id 1", "direct"),
            "client",
        )
        self.assertIsNone(
            MODULE.process_role("cargo build --bin brawler-server", "direct")
        )

    def test_cpu_gate_is_unsupported_without_comparable_series(self) -> None:
        direct = {"cpu": {"aggregate_cpu_seconds": 1.0, "comparable": False}}
        routed = {"cpu": {"aggregate_cpu_seconds": 1.1, "comparable": True}}
        result = MODULE.compare_cpu(direct, routed)
        self.assertEqual(result["status"], "unsupported")
        self.assertNotIn("regression_ratio", result)

    def test_rss_summary_is_independent_of_cpu_comparison_context(self) -> None:
        result = MODULE.rss_summary(
            {
                "client": {
                    "101": {"max_rss_kib": 12},
                    "102": {"max_rss_kib": 34},
                }
            }
        )
        self.assertEqual(
            result["client"], {"max_rss_kib": 34, "process_count": 2}
        )

    def test_cpu_cardinality_requires_every_expected_process(self) -> None:
        summary = {
            "comparable": True,
            "roles": {
                "supervisor": {"process_count": 1},
                "lobby": {"process_count": 1},
                "match": {"process_count": 1},
                "client": {"process_count": 2},
            },
        }
        result = MODULE.enforce_cpu_role_cardinality(summary, "routed")
        self.assertTrue(result["comparable"])
        summary = {
            "comparable": True,
            "roles": {
                "supervisor": {"process_count": 1},
                "lobby": {"process_count": 1},
                "client": {"process_count": 2},
            },
        }
        result = MODULE.enforce_cpu_role_cardinality(summary, "routed")
        self.assertFalse(result["comparable"])
        self.assertIn("expected exact roles", result["cardinality_error"])

    def test_cpu_gate_enforces_twenty_percent_only_after_comparison(self) -> None:
        direct = {"cpu": {"aggregate_cpu_seconds": 10.0, "comparable": True}}
        routed = {"cpu": {"aggregate_cpu_seconds": 12.0, "comparable": True}}
        result = MODULE.compare_cpu(direct, routed)
        self.assertEqual(result["status"], "pass")
        self.assertAlmostEqual(result["regression_ratio"], 0.2)
        routed["cpu"]["aggregate_cpu_seconds"] = 12.01
        self.assertEqual(MODULE.compare_cpu(direct, routed)["status"], "fail")

    def test_bandwidth_is_directional_and_excludes_routed_overhead(self) -> None:
        direct = {"ingress_bytes": 1000, "egress_bytes": 2000}
        routed = {
            "ingress_bytes": 1099,
            "egress_bytes": 2201,
            "overhead": {"ipc": {"bytes_total": 999_999}},
        }
        result = MODULE.compare_directional_bandwidth(direct, routed)
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["directions"]["ingress"]["status"], "pass")
        self.assertEqual(result["directions"]["egress"]["status"], "fail")
        self.assertNotIn("bytes_total", result)

    def test_bandwidth_is_unsupported_for_zero_direct_direction(self) -> None:
        result = MODULE.compare_directional_bandwidth(
            {"ingress_bytes": 0, "egress_bytes": 100},
            {"ingress_bytes": 1, "egress_bytes": 101},
        )
        self.assertEqual(result["status"], "unsupported")
        self.assertEqual(result["directions"]["ingress"]["status"], "unsupported")
        self.assertEqual(result["directions"]["egress"]["status"], "pass")

    def test_routed_bandwidth_compares_match_only_but_keeps_total_overhead(self) -> None:
        path = ROOT / "target" / "paired-routing-metrics-test.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        public_ingress = {"bytes": 1_420, "datagrams": 10, "frames": 10}
        public_egress = {"bytes": 2_840, "datagrams": 20, "frames": 20}
        try:
            path.write_text(
                json.dumps(
                    {
                        "traffic": {
                            "public_ingress": public_ingress,
                            "public_egress": public_egress,
                            "inner_ingress": {"bytes": 1_000, "datagrams": 10, "frames": 10},
                            "inner_egress": {"bytes": 2_000, "datagrams": 20, "frames": 20},
                            "match_inner_ingress": {
                                "bytes": 900,
                                "datagrams": 8,
                                "frames": 8,
                            },
                            "match_inner_egress": {
                                "bytes": 1_800,
                                "datagrams": 16,
                                "frames": 16,
                            },
                            "ipc_to_worker": {"bytes": 3, "datagrams": 0, "frames": 1},
                            "ipc_from_worker": {"bytes": 4, "datagrams": 0, "frames": 1},
                        }
                    }
                ),
                encoding="utf-8",
            )
            result = MODULE.read_routed_bandwidth({"artifacts": {"metrics": str(path)}})
            self.assertEqual(result["status"], "measured")
            self.assertEqual(result["ingress_bytes"], 900)
            self.assertEqual(result["egress_bytes"], 1_800)
            self.assertEqual(result["overhead"]["ingress"]["status"], "pass")
            self.assertEqual(result["overhead"]["egress"]["status"], "pass")
        finally:
            path.unlink(missing_ok=True)

    def test_closeout_parser_rejects_duplicates_and_reads_transport_fields(self) -> None:
        path = ROOT / "target" / "paired-parser-test.closeout"
        path.parent.mkdir(parents=True, exist_ok=True)
        try:
            path.write_text(
                "transport_bytes_received=12\ntransport_bytes_sent=34\n", encoding="utf-8"
            )
            fields, error = MODULE.parse_closeout(path)
            self.assertIsNone(error)
            assert fields is not None
            self.assertEqual(MODULE.numeric_field(fields, "transport_bytes_sent"), 34)
            path.write_text(
                "transport_bytes_received=12\ntransport_bytes_received=34\n", encoding="utf-8"
            )
            _fields, error = MODULE.parse_closeout(path)
            self.assertIsNotNone(error)
        finally:
            path.unlink(missing_ok=True)

    def test_common_window_requires_exact_positive_tick_interval_and_deltas(self) -> None:
        path = ROOT / "target" / "paired-common-window-test.marker"
        path.parent.mkdir(parents=True, exist_ok=True)
        try:
            path.write_text(
                "\n".join(
                    [
                        "schema=brawler-common-window-v1",
                        "status=complete",
                        "role=server",
                        "run_id=pair-1",
                        "scenario_id=pair-1",
                        "scenario_revision=1",
                        "mode=wipeout",
                        "rules_profile=verification",
                        "network_profile=paired-m01-movement",
                        "protocol_version=1",
                        "registry_fingerprint=2",
                        "content_fingerprint=3",
                        "mode_definition_id=1",
                        "rules_revision=1",
                        "participant_count=2",
                        "result_kind=team-victory",
                        "result_team_a=1",
                        "result_team_b=0",
                        "start_tick=10",
                        "end_tick=30",
                        "tick_count=20",
                        "transport_bytes_sent_start=100",
                        "transport_bytes_sent_end=140",
                        "transport_bytes_received_start=200",
                        "transport_bytes_received_end=260",
                        "packets_sent_start=2",
                        "packets_sent_end=6",
                        "packets_received_start=3",
                        "packets_received_end=9",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            result = MODULE.read_common_window(path)
            self.assertEqual(result["status"], "measured")
            self.assertEqual(result["tick_count"], 20)
            self.assertEqual(result["transport_bytes_sent"], 40)
            self.assertEqual(result["transport_bytes_received"], 60)
            self.assertEqual(result["packets_sent"], 4)
            self.assertEqual(result["packets_received"], 6)
            routed = dict(result, role="match", start_tick=40, end_tick=60)
            self.assertEqual(MODULE.compare_common_windows(result, routed)["status"], "measured")
            self.assertEqual(
                MODULE.compare_common_windows(result, dict(routed, role="client"))["status"],
                "unsupported",
            )
            zero_delta = path.read_text(encoding="utf-8").replace(
                "transport_bytes_sent_end=140", "transport_bytes_sent_end=100"
            )
            path.write_text(zero_delta, encoding="utf-8")
            self.assertEqual(MODULE.read_common_window(path)["status"], "unsupported")
            zero_fingerprint = path.read_text(encoding="utf-8").replace(
                "transport_bytes_sent_end=140", "transport_bytes_sent_end=140"
            ).replace("registry_fingerprint=2", "registry_fingerprint=0")
            path.write_text(zero_fingerprint, encoding="utf-8")
            parsed = MODULE.read_common_window(path)
            self.assertEqual(parsed["status"], "unsupported")
            self.assertIn("zero protocol/content fingerprint", parsed["parse_error"])
            zero_content = path.read_text(encoding="utf-8").replace(
                "content_fingerprint=3", "content_fingerprint=0"
            )
            path.write_text(zero_content, encoding="utf-8")
            parsed = MODULE.read_common_window(path)
            self.assertEqual(parsed["status"], "unsupported")
            self.assertIn("zero protocol/content fingerprint", parsed["parse_error"])
            path.write_text(
                zero_delta.replace("tick_count=20", "tick_count=19"),
                encoding="utf-8",
            )
            self.assertEqual(MODULE.read_common_window(path)["status"], "unsupported")
        finally:
            path.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
