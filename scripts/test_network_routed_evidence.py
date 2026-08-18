#!/usr/bin/env python3
"""Focused unit tests for the local routed-process evidence parser.

Run with ``python3 -m unittest scripts/test_network_routed_evidence.py`` from the repository root.
The tests intentionally do not launch processes or execute multiple routed cycles.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path
from unittest.mock import patch
import unittest


ROOT = Path(__file__).resolve().parent.parent
MODULE_PATH = ROOT / "scripts" / "network-routed-evidence.py"
SPEC = importlib.util.spec_from_file_location("network_routed_evidence", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class EvidenceParserTests(unittest.TestCase):
    @staticmethod
    def lifecycle_log(*events: str) -> str:
        return "\n".join(
            [
                "brawler-supervisor worker ready worker=10 elapsed_ms=12",
                *events,
                "brawler-supervisor worker stop-requested worker=10 stop_id=71 elapsed_ms=401",
                "brawler-supervisor worker stop-sent worker=10 stop_id=71 elapsed_ms=402",
                "brawler-supervisor worker reaped worker=10 success=true code=Some(0) elapsed_ms=403",
                "brawler-supervisor worker stopped worker=10 forced=false elapsed_ms=403",
                "brawler-supervisor worker cleaned worker=10 elapsed_ms=403",
            ]
        )

    @staticmethod
    def match_events(worker: int = 20) -> tuple[str, ...]:
        return (
            f"brawler-supervisor worker spawned worker={worker} pid=200 elapsed_ms=100",
            f"brawler-supervisor worker ready worker={worker} elapsed_ms=112",
            f"brawler-supervisor worker result-received worker={worker}",
            f"brawler-supervisor worker reaped worker={worker} success=true code=Some(0) elapsed_ms=300",
            f"brawler-supervisor worker stopped worker={worker} forced=false elapsed_ms=300",
            f"brawler-supervisor worker cleaned worker={worker} elapsed_ms=300",
        )

    def test_process_role_requires_brawler_binary_as_executable(self) -> None:
        self.assertEqual(
            MODULE.process_role(
                "/Users/boyd/wip/brawler/target/debug/brawler-server "
                "--role match --worker-id 42"
            ),
            "match",
        )
        self.assertEqual(
            MODULE.process_role("target/debug/brawler-server --role=lobby"),
            "lobby",
        )
        self.assertEqual(
            MODULE.process_role("/Users/boyd/wip/brawler/target/debug/brawler-supervisor --bind 127.0.0.1:5000"),
            "supervisor",
        )
        self.assertIsNone(
            MODULE.process_role("cargo build --bin brawler-server --bin brawler-supervisor")
        )
        self.assertIsNone(
            MODULE.process_role(
                "python3 scripts/network-routed-evidence.py --worker-executable brawler-server"
            )
        )
        self.assertIsNone(
            MODULE.process_role("sh -c 'target/debug/brawler-server --role match'")
        )

    def test_lifecycle_separates_persistent_lobby_from_match(self) -> None:
        stderr = self.lifecycle_log(*self.match_events())
        lifecycle = MODULE.parse_lifecycle(stderr)
        self.assertEqual(lifecycle["lobby_ready"], 1)
        self.assertEqual(lifecycle["match_spawned"], 1)
        self.assertEqual(lifecycle["match_ready"], 1)
        self.assertEqual(lifecycle["match_result_received"], 1)
        self.assertEqual(lifecycle["match_reaped"], 1)
        self.assertEqual(lifecycle["match_stopped_graceful"], 1)
        self.assertEqual(lifecycle["match_cleaned"], 1)
        self.assertEqual(lifecycle["lobby_shutdown_status"], "pass")
        self.assertEqual(lifecycle["lobby_reaped"], 1)
        self.assertEqual(lifecycle["lobby_stopped_graceful"], 1)
        self.assertEqual(lifecycle["lobby_cleaned"], 1)
        self.assertEqual(
            lifecycle["observed_lobby_sequence"],
            ["ready", "stop_requested", "stop_sent", "reaped", "stopped", "cleaned"],
        )
        self.assertEqual(lifecycle["ready_ms"], [12])
        self.assertEqual(
            lifecycle["observed_match_sequence"],
            ["spawned", "ready", "result_received", "reaped", "stopped", "cleaned"],
        )
        self.assertEqual(lifecycle["sequence_failures"], [])

    def test_lifecycle_requires_normal_persistent_lobby_shutdown(self) -> None:
        events = list(self.match_events())
        # Remove the lobby's StopSent record from the lifecycle suffix while retaining the rest of
        # the match and lobby records.  The parser must fail closed on the missing ordered event.
        stderr = self.lifecycle_log(*events)
        stderr = stderr.replace(
            "brawler-supervisor worker stop-sent worker=10 stop_id=71 elapsed_ms=402\n", ""
        )
        lifecycle = MODULE.parse_lifecycle(stderr)
        self.assertEqual(lifecycle["lobby_shutdown_status"], "fail")
        self.assertEqual(
            lifecycle["observed_lobby_sequence"],
            ["ready", "stop_requested", "reaped", "stopped", "cleaned"],
        )
        self.assertTrue(
            any("persistent lobby lifecycle order" in failure
                for failure in lifecycle["sequence_failures"])
        )

    def test_lifecycle_rejects_failed_or_forced_persistent_lobby_shutdown(self) -> None:
        stderr = self.lifecycle_log(*self.match_events())
        stderr = stderr.replace(
            "brawler-supervisor worker reaped worker=10 success=true code=Some(0) elapsed_ms=403",
            "brawler-supervisor worker reaped worker=10 success=false code=Some(1) elapsed_ms=403",
        ).replace(
            "brawler-supervisor worker stopped worker=10 forced=false elapsed_ms=403",
            "brawler-supervisor worker stopped worker=10 forced=true elapsed_ms=403",
        )
        lifecycle = MODULE.parse_lifecycle(stderr)
        self.assertEqual(lifecycle["lobby_shutdown_status"], "fail")
        self.assertEqual(lifecycle["lobby_reaped"], 0)
        self.assertEqual(lifecycle["lobby_failed_reaped"], ["10"])
        self.assertEqual(lifecycle["lobby_stopped_graceful"], 0)
        self.assertTrue(
            any("reaped unsuccessfully" in failure
                for failure in lifecycle["sequence_failures"])
        )

    def test_lifecycle_rejects_missing_result(self) -> None:
        events = list(self.match_events())
        events.remove("brawler-supervisor worker result-received worker=20")
        lifecycle = MODULE.parse_lifecycle(self.lifecycle_log(*events))
        self.assertEqual(lifecycle["match_result_received"], 0)
        self.assertTrue(
            any("expected ['spawned', 'ready', 'result_received'" in failure
                for failure in lifecycle["sequence_failures"])
        )

    def test_lifecycle_rejects_duplicate_match_spawn(self) -> None:
        events = list(self.match_events())
        events.insert(
            1,
            "brawler-supervisor worker spawned worker=21 pid=201 elapsed_ms=101",
        )
        lifecycle = MODULE.parse_lifecycle(self.lifecycle_log(*events))
        self.assertEqual(lifecycle["match_spawned"], 2)
        self.assertEqual(lifecycle["dynamic_match_workers"], ["20", "21"])
        self.assertTrue(
            any("exactly one dynamic match spawn" in failure
                for failure in lifecycle["sequence_failures"])
        )

    def test_lifecycle_rejects_result_from_wrong_worker(self) -> None:
        events = list(self.match_events())
        events[2] = "brawler-supervisor worker result-received worker=99"
        lifecycle = MODULE.parse_lifecycle(self.lifecycle_log(*events))
        self.assertEqual(lifecycle["match_result_received"], 0)
        self.assertIn("99", lifecycle["result_workers"])
        self.assertTrue(
            any("non-match worker" in failure for failure in lifecycle["sequence_failures"])
        )

    def test_lifecycle_rejects_wrong_event_order(self) -> None:
        events = list(self.match_events())
        events[1], events[2] = events[2], events[1]
        lifecycle = MODULE.parse_lifecycle(self.lifecycle_log(*events))
        self.assertEqual(
            lifecycle["observed_match_sequence"],
            ["spawned", "result_received", "ready", "reaped", "stopped", "cleaned"],
        )
        self.assertTrue(
            any("dynamic match lifecycle order" in failure
                for failure in lifecycle["sequence_failures"])
        )

    def test_lifecycle_lobby_ready_is_one_shot(self) -> None:
        stderr = self.lifecycle_log(
            "brawler-supervisor worker ready worker=10 elapsed_ms=13",
            *self.match_events(),
        )
        lifecycle = MODULE.parse_lifecycle(stderr)
        self.assertEqual(lifecycle["lobby_ready"], 2)
        self.assertTrue(
            any("exactly one persistent lobby Ready" in failure
                for failure in lifecycle["sequence_failures"])
        )

    def test_metrics_check_uses_live_capability_count(self) -> None:
        metrics = {
            "workers": 0,
            "routes": 0,
            "capabilities": 2,  # Revoked negative records remain for bounded replay handling.
            "live_capabilities": 0,
            "process_workers": 0,
            "packet_current_frames": 0,
            "packet_current_bytes": 0,
            "control_current_frames": 0,
            "control_current_bytes": 0,
            "packet_dropped_newest": 0,
            "control_rejected": 0,
            "source_limited": 0,
            "capabilities_revoked": 2,
            "runtime_dir_entries": 0,
            "errors": {},
        }
        self.assertEqual(MODULE.metrics_failures(metrics), [])
        metrics["errors"] = {"Revoked": 32}
        self.assertEqual(MODULE.metrics_failures(metrics), [])
        metrics["errors"] = {"Revoked": 33}
        self.assertTrue(
            any("terminal allowance" in failure for failure in MODULE.metrics_failures(metrics))
        )
        metrics["errors"] = {"Revoked": 1, "PacketQueueFull": 1}
        self.assertTrue(
            any("unexpected categories" in failure for failure in MODULE.metrics_failures(metrics))
        )

    def test_success_marker_is_fresh_lobby_marker(self) -> None:
        self.assertIn(
            "lobby-to-match-to-fresh-lobby",
            MODULE.ROUTED_SUCCESS_MARKER,
        )
        self.assertTrue(
            MODULE.ROUTED_SUCCESS_MARKER
            in (
                "brawler routed network: two-client "
                "lobby-to-match-to-fresh-lobby transition passed"
            )
        )

    def test_owner_boundary_measurement_is_diagnostic_without_inventing_a_latency_gate(self) -> None:
        traffic = {
            "public_ingress": {"datagrams": 1, "frames": 1, "bytes": 43},
            "public_egress": {"datagrams": 1, "frames": 1, "bytes": 43},
            "inner_ingress": {"datagrams": 1, "frames": 1, "bytes": 1},
            "inner_egress": {"datagrams": 1, "frames": 1, "bytes": 1},
            "ipc_to_worker": {"datagrams": 0, "frames": 1, "bytes": 5},
            "ipc_from_worker": {"datagrams": 0, "frames": 1, "bytes": 5},
        }
        metrics = {
            "traffic": traffic,
            "latency": {
                "public_receive_to_packet_ipc_enqueue": {"samples": 9, "p95_us": 1},
                "worker_packet_to_public_send": {"samples": 8, "p95_us": 1},
            },
        }
        measurement = MODULE.owner_boundary_measurement(metrics)
        self.assertIsNotNone(measurement)
        assert measurement is not None
        self.assertEqual(measurement["status"], "measured_diagnostic")
        self.assertEqual(measurement["threshold_status"], "diagnostic_only")
        self.assertEqual(measurement["paired_sample_count"], 8)
        self.assertEqual(measurement["public_traffic_validation"]["status"], "pass")

    def test_public_formula_rejects_wrong_bytes(self) -> None:
        traffic = {
            "public_ingress": {"datagrams": 2, "frames": 2, "bytes": 101},
            "public_egress": {"datagrams": 1, "frames": 1, "bytes": 43},
            "inner_ingress": {"datagrams": 2, "frames": 2, "bytes": 16},
            "inner_egress": {"datagrams": 1, "frames": 1, "bytes": 1},
            "ipc_to_worker": {"datagrams": 0, "frames": 2, "bytes": 0},
            "ipc_from_worker": {"datagrams": 0, "frames": 2, "bytes": 0},
        }
        validation = MODULE.validate_public_traffic(traffic)
        self.assertEqual(validation["status"], "fail")
        self.assertTrue(
            any("public_ingress.bytes expected" in failure for failure in validation["failures"])
        )

    def test_public_formula_rejects_malformed_datagram_count(self) -> None:
        traffic = {
            "public_ingress": {"datagrams": 2, "frames": 2, "bytes": 86},
            "public_egress": {"datagrams": 1, "frames": 1, "bytes": 43},
            "inner_ingress": {"datagrams": 1, "frames": 1, "bytes": 44},
            "inner_egress": {"datagrams": 1, "frames": 1, "bytes": 1},
        }
        validation = MODULE.validate_public_traffic(traffic)
        self.assertEqual(validation["status"], "fail")
        self.assertTrue(
            any("datagrams does not match" in failure for failure in validation["failures"])
        )

    def test_ipc_exact_overhead_is_explicitly_unsupported(self) -> None:
        traffic = {
            "public_ingress": {"datagrams": 0, "frames": 0, "bytes": 0},
            "public_egress": {"datagrams": 0, "frames": 0, "bytes": 0},
            "inner_ingress": {"datagrams": 0, "frames": 0, "bytes": 0},
            "inner_egress": {"datagrams": 0, "frames": 0, "bytes": 0},
            "ipc_to_worker": {"datagrams": 0, "frames": 10_000, "bytes": 420_000},
            "ipc_from_worker": {"datagrams": 0, "frames": 10_000, "bytes": 420_000},
        }
        metrics = {
            "traffic": traffic,
            "latency": {
                "public_receive_to_packet_ipc_enqueue": {"samples": 10_000, "p95_us": 2_001},
                "worker_packet_to_public_send": {"samples": 10_000, "p95_us": 1},
            },
        }
        measurement = MODULE.owner_boundary_measurement(metrics)
        self.assertIsNotNone(measurement)
        assert measurement is not None
        self.assertEqual(measurement["threshold_status"], "diagnostic_only")
        self.assertEqual(
            measurement["public_traffic_validation"]["ipc_exact_overhead_status"],
            "unsupported",
        )

    def test_lifecycle_duration_is_unsupported_without_stop_timestamp(self) -> None:
        lifecycle = MODULE.parse_lifecycle(self.lifecycle_log(*self.match_events()))
        self.assertEqual(lifecycle["graceful_stop_reap_duration_status"], "unsupported")
        self.assertIsNone(lifecycle["graceful_stop_reap_duration_ms"])

    def test_lifecycle_correlates_request_send_reap_and_cleanup(self) -> None:
        events = list(self.match_events())
        events[2] = "brawler-supervisor worker result-received worker=20 elapsed_ms=200 ts_ms=1200"
        events.insert(
            3,
            "brawler-supervisor worker stop-requested worker=20 stop_id=77 elapsed_ms=201 ts_ms=1201",
        )
        events.insert(
            4,
            "brawler-supervisor worker stop-sent worker=20 stop_id=77 elapsed_ms=202 ts_ms=1202",
        )
        events[5] = "brawler-supervisor worker reaped worker=20 success=true code=Some(0) elapsed_ms=300 ts_ms=1300"
        events[6] = "brawler-supervisor worker stopped worker=20 forced=false elapsed_ms=301 ts_ms=1301"
        events[7] = "brawler-supervisor worker cleaned worker=20 elapsed_ms=302 ts_ms=1302"
        lifecycle = MODULE.parse_lifecycle(self.lifecycle_log(*events))
        self.assertEqual(lifecycle["observed_match_sequence"][3:5], ["stop_requested", "stop_sent"])
        self.assertEqual(lifecycle["graceful_stop_reap_duration_status"], "measured")
        self.assertEqual(lifecycle["graceful_stop_reap_duration_ms"], 99)
        self.assertEqual(lifecycle["graceful_stop_cleanup_duration_ms"], 101)

    def test_handoff_requires_two_distinct_clients_and_correlates_request_id(self) -> None:
        log = "\n".join(
            [
                "brawler-supervisor timing allocation-accepted request_id=41 worker=20 ts_ms=1000 elapsed_ms=10",
                "brawler-client timing handoff-connected client_id=1 request_id=41 ts_ms=2100",
                "brawler-client timing handoff-connected client_id=2 request_id=41 ts_ms=2200",
            ]
        )
        handoff = MODULE.parse_handoff_timing(log)
        self.assertEqual(handoff["status"], "measured")
        self.assertEqual(handoff["sample_count"], 1)
        self.assertEqual(handoff["handoff_ms"], [1200])
        self.assertEqual(handoff["samples"][0]["worker_id"], "20")

    def test_handoff_rejects_unknown_request_duplicate_client_and_secret_marker(self) -> None:
        log = "\n".join(
            [
                "brawler-supervisor timing allocation-accepted request_id=41 worker=20 ts_ms=1000 elapsed_ms=10",
                "brawler-client timing handoff-connected client_id=1 request_id=41 ts_ms=1100 capability=raw",
                "brawler-client timing handoff-connected client_id=1 request_id=41 ts_ms=1200",
                "brawler-client timing handoff-connected client_id=9 request_id=99 ts_ms=1200",
            ]
        )
        handoff = MODULE.parse_handoff_timing(log)
        self.assertEqual(handoff["status"], "invalid")
        self.assertEqual(handoff["redaction_status"], "fail")
        self.assertTrue(any("distinct match connections" in failure for failure in handoff["failures"]))
        self.assertTrue(any("unknown request" in failure for failure in handoff["failures"]))
        self.assertTrue(any("secret-bearing" in failure for failure in handoff["failures"]))

    def test_handoff_rejects_negative_cross_process_timestamp(self) -> None:
        log = "\n".join(
            [
                "brawler-supervisor timing allocation-accepted request_id=41 worker=20 ts_ms=2000 elapsed_ms=10",
                "brawler-client timing handoff-connected client_id=1 request_id=41 ts_ms=1900",
                "brawler-client timing handoff-connected client_id=2 request_id=41 ts_ms=2100",
            ]
        )
        handoff = MODULE.parse_handoff_timing(log)
        self.assertEqual(handoff["status"], "invalid")
        self.assertTrue(any("connected before allocation" in failure for failure in handoff["failures"]))

    def test_lifecycle_duration_enforces_correlated_graceful_limit(self) -> None:
        events = list(self.match_events())
        events[2] = "brawler-supervisor worker result-received worker=20 elapsed_ms=200"
        events.insert(3, "brawler-supervisor worker stop-sent worker=20 elapsed_ms=201")
        events[4] = "brawler-supervisor worker reaped worker=20 success=true code=Some(0) elapsed_ms=2401"
        events[5] = "brawler-supervisor worker stopped worker=20 forced=false elapsed_ms=2401"
        events[6] = "brawler-supervisor worker cleaned worker=20 elapsed_ms=2401"
        lifecycle = MODULE.parse_lifecycle(self.lifecycle_log(*events))
        self.assertEqual(lifecycle["graceful_stop_reap_duration_status"], "measured")
        self.assertEqual(lifecycle["graceful_stop_reap_duration_ms"], 2200)

    def test_mode_profiles_are_explicit_and_both_means_cycles_per_mode(self) -> None:
        with patch.object(MODULE.sys, "argv", ["evidence", "--mode", "both", "--cycles", "25"]):
            args = MODULE.parse_args()
        self.assertEqual(args.mode, "both")
        self.assertEqual(args.cycles, 25)

    def test_crash_restart_is_a_distinct_evidence_profile(self) -> None:
        with patch.object(
            MODULE.sys, "argv", ["evidence", "--mode", "crash-restart", "--cycles", "20"]
        ):
            args = MODULE.parse_args()
        self.assertEqual(args.mode, "crash-restart")
        self.assertEqual(args.cycles, 20)


if __name__ == "__main__":
    unittest.main()
