#!/usr/bin/env python3
"""Paired direct-UDP versus routed-process M01 measurement.

This harness runs the existing direct and routed verification launchers sequentially on the same
host and source tree.  It samples Brawler descendants with ``ps`` at 10 Hz, reads the direct
server's process-metrics closeout, and reads the routed supervisor's directional counters.

Only the common opaque gameplay/Netcode byte boundary is compared:

* direct ``server.closeout`` transport bytes;
* routed ``traffic.match_inner_ingress`` and ``traffic.match_inner_egress`` bytes.

Public envelope and framed IPC bytes are retained as routed-only overhead diagnostics.  They are
never compared with direct gameplay bytes.  Missing or non-comparable samples are reported as
``unsupported`` rather than being converted into a fabricated pass or failure.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import os
from pathlib import Path
import platform
import re
import shlex
import signal
import subprocess
import sys
import time
from typing import Any


SCHEMA = "brawler-paired-evidence-v1"
DEFAULT_PAIRS = 3
MAX_PAIRS = 3
DEFAULT_TIMEOUT_SECONDS = 90
SAMPLE_INTERVAL_SECONDS = 0.1
CPU_REGRESSION_LIMIT = 0.20
BANDWIDTH_REGRESSION_LIMIT = 0.10
# A cold measurement build may compile the optional Lightyear metrics graph. Keep the runtime
# watchdog bounded while allowing the first build to finish; subsequent pairs use Cargo's cache.
BUILD_AND_CLEANUP_MARGIN_SECONDS = 180
PUBLIC_ENVELOPE_BYTES = 42
ROUTED_SUCCESS_MARKER = (
    "brawler routed network: two-client lobby-to-match-to-fresh-lobby transition passed"
)
DIRECT_SUCCESS_MARKER = "brawler network: closeout reports validated"
COMPARABLE_SIMULATION_TICKS = 4_000
EXPECTED_ROLE_COUNTS = {
    "direct": {"server": 1, "client": 2},
    "routed": {"supervisor": 1, "lobby": 1, "match": 1, "client": 2},
}


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run bounded paired direct/routed M01 CPU and gameplay-bandwidth evidence."
    )
    parser.add_argument(
        "--pairs",
        type=int,
        default=int(os.environ.get("BRAWLER_PAIRED_EVIDENCE_PAIRS", str(DEFAULT_PAIRS))),
        help="number of sequential direct/routed pairs (1..3; default: 3)",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=int(
            os.environ.get(
                "BRAWLER_PAIRED_EVIDENCE_TIMEOUT_SECONDS", str(DEFAULT_TIMEOUT_SECONDS)
            )
        ),
        help="per-launcher runtime watchdog in seconds (1..120; default: 90)",
    )
    parser.add_argument(
        "--mode",
        choices=("wipeout", "hot-zone", "heist"),
        default=os.environ.get("BRAWLER_PAIRED_EVIDENCE_MODE", "wipeout"),
        help="existing verification mode used by both launchers (default: wipeout)",
    )
    parser.add_argument(
        "--profile",
        choices=("movement",),
        default="movement",
        help="existing comparable verification profile (default: movement)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="summary JSON path (default: target/paired-evidence-<UTC timestamp>.json)",
    )
    parser.add_argument(
        "--keep-artifacts",
        action="store_true",
        help="retain launcher logs and per-run snapshots next to the summary",
    )
    return parser.parse_args(argv)


def p95(values: list[float]) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * 0.95) - 1)]


def parse_cpu_time(value: str) -> float | None:
    """Parse macOS/Linux ``ps time`` values into seconds.

    macOS commonly emits ``MM:SS.hh`` while Linux emits ``HH:MM:SS`` (and can include a
    fractional suffix).  The parser is intentionally strict enough to reject an unavailable
    value instead of treating it as zero CPU.
    """

    value = value.strip()
    if not value or value in {"-", "?"}:
        return None
    match = re.fullmatch(r"(?:(\d+)-)?(\d+):(\d{2}):(\d{2})(?:\.(\d+))?", value)
    if match:
        days, hours, minutes, seconds, fraction = match.groups()
        total = (
            int(days or 0) * 86_400
            + int(hours) * 3_600
            + int(minutes) * 60
            + int(seconds)
        )
        if fraction:
            total += int(fraction) / (10 ** len(fraction))
        return float(total)
    match = re.fullmatch(r"(\d+):(\d{2})(?:\.(\d+))?", value)
    if match:
        minutes, seconds, fraction = match.groups()
        total = int(minutes) * 60 + int(seconds)
        if fraction:
            total += int(fraction) / (10 ** len(fraction))
        return float(total)
    return None


def ps_rows() -> list[tuple[int, int, int, float, str]]:
    """Read portable PID/parent/RSS/CPU/argv rows for process sampling."""

    try:
        result = subprocess.run(
            ["ps", "-ax", "-o", "pid=", "-o", "ppid=", "-o", "rss=", "-o", "time=", "-o", "command="],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return []
    rows: list[tuple[int, int, int, float, str]] = []
    for line in result.stdout.splitlines():
        fields = line.strip().split(maxsplit=4)
        if len(fields) < 5:
            continue
        cpu = parse_cpu_time(fields[3])
        if cpu is None:
            continue
        try:
            rows.append((int(fields[0]), int(fields[1]), int(fields[2]), cpu, fields[4]))
        except ValueError:
            continue
    return rows


def process_tree(root_pid: int, rows: list[tuple[int, int, int, float, str]]) -> set[int]:
    children: dict[int, list[int]] = {}
    for pid, ppid, _rss, _cpu, _command in rows:
        children.setdefault(ppid, []).append(pid)
    tree = {root_pid}
    pending = [root_pid]
    while pending:
        parent = pending.pop()
        for child in children.get(parent, []):
            if child not in tree:
                tree.add(child)
                pending.append(child)
    return tree


def process_role(command: str, topology: str) -> str | None:
    """Classify only an actual Brawler executable, never a cargo/shell command."""

    try:
        argv = shlex.split(command)
    except ValueError:
        return None
    if not argv:
        return None
    executable = Path(argv[0]).name.lower()
    args = [argument.lower() for argument in argv[1:]]
    if executable == "brawler-client":
        return "client"
    if topology == "routed" and executable == "brawler-supervisor":
        return "supervisor"
    if executable != "brawler-server":
        return None
    role: str | None = None
    for index, argument in enumerate(args):
        if argument == "--role" and index + 1 < len(args):
            role = args[index + 1]
            break
        if argument.startswith("--role="):
            role = argument.partition("=")[2]
            break
    if topology == "routed":
        return role if role in {"lobby", "match"} else None
    return "server" if role is None else None


def sample_processes(
    root_pid: int,
    topology: str,
    samples: dict[str, dict[str, dict[str, Any]]],
) -> None:
    rows = ps_rows()
    by_pid = {pid: (rss, cpu, command) for pid, _ppid, rss, cpu, command in rows}
    for pid in process_tree(root_pid, rows):
        row = by_pid.get(pid)
        if row is None:
            continue
        rss, cpu, command = row
        role = process_role(command, topology)
        if role is None:
            continue
        process = samples.setdefault(role, {}).setdefault(
            str(pid),
            {
                "role": role,
                "samples": [],
                "first_cpu_seconds": None,
                "last_cpu_seconds": None,
                "cpu_sample_count": 0,
                "max_rss_kib": 0,
            },
        )
        process["samples"].append(
            {"monotonic_seconds": time.monotonic(), "cpu_seconds": cpu, "rss_kib": rss}
        )
        process["first_cpu_seconds"] = cpu if process["first_cpu_seconds"] is None else process["first_cpu_seconds"]
        process["last_cpu_seconds"] = cpu
        process["cpu_sample_count"] += 1
        process["max_rss_kib"] = max(process["max_rss_kib"], rss)


def terminate_owned_processes(pids: set[int]) -> list[int]:
    rows = {pid: command for pid, _ppid, _rss, _cpu, command in ps_rows()}
    live: list[int] = []
    for pid in sorted(pids):
        if pid not in rows:
            continue
        try:
            os.kill(pid, signal.SIGTERM)
            live.append(pid)
        except (ProcessLookupError, PermissionError):
            continue
    return live


def parse_closeout(path: Path) -> tuple[dict[str, str] | None, str | None]:
    try:
        contents = path.read_text(encoding="utf-8")
    except OSError as error:
        return None, str(error)
    fields: dict[str, str] = {}
    for line in contents.splitlines():
        if not line:
            continue
        if "=" not in line:
            return None, f"malformed closeout line: {line!r}"
        key, value = line.split("=", 1)
        if not key or not value or key in fields:
            return None, f"invalid or duplicate closeout field: {key!r}"
        fields[key] = value
    return fields, None


def numeric_field(fields: dict[str, str], key: str) -> int | None:
    value = fields.get(key)
    if value is None or not re.fullmatch(r"\d+", value):
        return None
    return int(value)


COMMON_WINDOW_NUMERIC_FIELDS = (
    "scenario_revision",
    "protocol_version",
    "registry_fingerprint",
    "content_fingerprint",
    "mode_definition_id",
    "rules_revision",
    "participant_count",
    "result_team_a",
    "result_team_b",
    "start_tick",
    "end_tick",
    "tick_count",
    "transport_bytes_sent_start",
    "transport_bytes_sent_end",
    "transport_bytes_received_start",
    "transport_bytes_received_end",
    "packets_sent_start",
    "packets_sent_end",
    "packets_received_start",
    "packets_received_end",
)


def read_common_window(path: Path) -> dict[str, Any]:
    """Read one authoritative Active->Completed marker, failing closed on drift."""

    fields, error = parse_closeout(path)
    result: dict[str, Any] = {
        "status": "unsupported",
        "path": str(path),
        "parse_error": error,
    }
    if fields is None:
        return result
    if fields.get("schema") != "brawler-common-window-v1":
        result["parse_error"] = "unknown common-window schema"
        return result
    if fields.get("status") != "complete":
        result["parse_error"] = "common-window marker is not complete"
        return result
    required_identity = (
        "role",
        "run_id",
        "scenario_id",
        "mode",
        "rules_profile",
        "network_profile",
        "result_kind",
    )
    missing_identity = [key for key in required_identity if not fields.get(key)]
    if missing_identity:
        result["parse_error"] = (
            "common-window marker is missing identity fields: "
            + ", ".join(missing_identity)
        )
        return result
    missing = [key for key in COMMON_WINDOW_NUMERIC_FIELDS if key not in fields]
    if missing:
        result["parse_error"] = f"common-window marker is missing fields: {', '.join(missing)}"
        return result
    values = {key: numeric_field(fields, key) for key in COMMON_WINDOW_NUMERIC_FIELDS}
    if any(value is None for value in values.values()):
        result["parse_error"] = "common-window marker has a non-numeric field"
        return result
    assert all(isinstance(value, int) for value in values.values())
    if values["registry_fingerprint"] == 0 or values["content_fingerprint"] == 0:
        result["parse_error"] = "common-window marker has a zero protocol/content fingerprint"
        return result
    start_tick = values["start_tick"]
    end_tick = values["end_tick"]
    tick_count = values["tick_count"]
    if end_tick < start_tick or tick_count != end_tick - start_tick or tick_count <= 0:
        result["parse_error"] = "common-window tick bounds are not a positive exact interval"
        return result
    for direction in ("sent", "received"):
        if values[f"transport_bytes_{direction}_end"] < values[f"transport_bytes_{direction}_start"]:
            result["parse_error"] = f"common-window transport {direction} counters are not monotonic"
            return result
    for direction in ("sent", "received"):
        if values[f"packets_{direction}_end"] < values[f"packets_{direction}_start"]:
            result["parse_error"] = f"common-window packet {direction} counters are not monotonic"
            return result
        byte_delta = (
            values[f"transport_bytes_{direction}_end"]
            - values[f"transport_bytes_{direction}_start"]
        )
        packet_delta = (
            values[f"packets_{direction}_end"] - values[f"packets_{direction}_start"]
        )
        if byte_delta <= 0 or packet_delta <= 0:
            result["parse_error"] = (
                f"common-window {direction} requires positive transport byte and packet deltas"
            )
            return result
    result.update(
        {
            "status": "measured",
            "role": fields.get("role"),
            "run_id": fields.get("run_id"),
            "scenario_id": fields.get("scenario_id"),
            "mode": fields.get("mode"),
            "rules_profile": fields.get("rules_profile"),
            "network_profile": fields.get("network_profile"),
            "scenario_revision": values["scenario_revision"],
            "protocol_version": values["protocol_version"],
            "registry_fingerprint": values["registry_fingerprint"],
            "content_fingerprint": values["content_fingerprint"],
            "mode_definition_id": values["mode_definition_id"],
            "rules_revision": values["rules_revision"],
            "participant_count": values["participant_count"],
            "result_kind": fields.get("result_kind"),
            "result_team_a": values["result_team_a"],
            "result_team_b": values["result_team_b"],
            "start_tick": start_tick,
            "end_tick": end_tick,
            "tick_count": tick_count,
            "transport_bytes_sent": values["transport_bytes_sent_end"] - values["transport_bytes_sent_start"],
            "transport_bytes_received": values["transport_bytes_received_end"] - values["transport_bytes_received_start"],
            "packets_sent": values["packets_sent_end"] - values["packets_sent_start"],
            "packets_received": values["packets_received_end"] - values["packets_received_start"],
            "raw": fields,
        }
    )
    return result


def compare_common_windows(direct: dict[str, Any], routed: dict[str, Any]) -> dict[str, Any]:
    result: dict[str, Any] = {
        "status": "unsupported",
        "direct": direct,
        "routed": routed,
        "comparison": "first authoritative Active->Completed fixed-tick interval",
    }
    if direct.get("status") != "measured" or routed.get("status") != "measured":
        result["reason"] = "one or both launchers did not emit a complete common-window marker"
        return result
    if direct.get("role") != "server" or routed.get("role") != "match":
        result["reason"] = "common-window roles must be direct server and routed match"
        return result
    # SimulationTick is process-local and starts before network admission, so absolute tick
    # bounds are not comparable across two independently launched topologies.  The interval
    # length is comparable: both markers are the first authoritative Active->Completed span.
    for key in (
        "run_id",
        "scenario_id",
        "scenario_revision",
        "mode",
        "rules_profile",
        "network_profile",
        "protocol_version",
        "registry_fingerprint",
        "content_fingerprint",
        "mode_definition_id",
        "rules_revision",
        "participant_count",
        "result_kind",
        "result_team_a",
        "result_team_b",
        "tick_count",
    ):
        if direct.get(key) != routed.get(key):
            result["reason"] = f"common-window {key} differs between direct and routed runs"
            return result
    result["status"] = "measured"
    return result


def process_cpu_summary(samples: dict[str, dict[str, dict[str, Any]]]) -> dict[str, Any]:
    by_role: dict[str, dict[str, Any]] = {}
    total = 0.0
    comparable = True
    for role, processes in sorted(samples.items()):
        role_total = 0.0
        role_comparable = bool(processes)
        process_summary: dict[str, Any] = {}
        for pid, process in sorted(processes.items()):
            count = int(process["cpu_sample_count"])
            first = process["first_cpu_seconds"]
            last = process["last_cpu_seconds"]
            delta = None
            if count >= 2 and isinstance(first, (int, float)) and isinstance(last, (int, float)):
                delta = max(0.0, float(last) - float(first))
                role_total += delta
            else:
                role_comparable = False
            process_summary[pid] = {
                "role": role,
                "cpu_sample_count": count,
                "first_cpu_seconds": first,
                "last_cpu_seconds": last,
                "cpu_seconds": delta,
                "max_rss_kib": process["max_rss_kib"],
                "samples": process["samples"],
            }
        comparable = comparable and role_comparable
        by_role[role] = {
            "cpu_seconds": role_total,
            "process_count": len(processes),
            "comparable": role_comparable,
            "processes": process_summary,
        }
        total += role_total
    return {
        "aggregate_cpu_seconds": total,
        "comparable": comparable and bool(samples),
        "roles": by_role,
    }


def enforce_cpu_role_cardinality(summary: dict[str, Any], topology: str) -> dict[str, Any]:
    expected = EXPECTED_ROLE_COUNTS[topology]
    observed = {
        role: int(value.get("process_count", 0))
        for role, value in summary.get("roles", {}).items()
    }
    cardinality_matches = observed == expected
    summary["expected_role_counts"] = expected
    summary["observed_role_counts"] = observed
    summary["cardinality_matches"] = cardinality_matches
    if not cardinality_matches:
        summary["comparable"] = False
        summary["cardinality_error"] = (
            f"expected exact roles {expected}, observed {observed}"
        )
    return summary


def rss_summary(samples: dict[str, dict[str, dict[str, Any]]]) -> dict[str, Any]:
    return {
        role: {
            "max_rss_kib": max((int(process["max_rss_kib"]) for process in processes.values()), default=None),
            "process_count": len(processes),
        }
        for role, processes in sorted(samples.items())
    }


def sample_counts(samples: dict[str, dict[str, dict[str, Any]]]) -> dict[str, int]:
    return {
        role: sum(int(process["cpu_sample_count"]) for process in processes.values())
        for role, processes in sorted(samples.items())
    }


def run_launcher(
    root: Path,
    topology: str,
    pair: int,
    mode: str,
    timeout_seconds: int,
    artifacts: Path,
    env_overrides: dict[str, str],
) -> dict[str, Any]:
    artifacts.mkdir(parents=True, exist_ok=True)
    stdout_path = artifacts / "stdout.log"
    stderr_path = artifacts / "stderr.log"
    samples: dict[str, dict[str, dict[str, Any]]] = {}
    environment = os.environ.copy()
    environment.update(env_overrides)
    script = root / ("scripts/network.sh" if topology == "direct" else "scripts/network-routed.sh")
    started = time.monotonic()
    timed_out = False
    seen_pids: set[int] = set()
    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
        "w", encoding="utf-8"
    ) as stderr:
        process = subprocess.Popen(
            ["bash", str(script)],
            cwd=root,
            env=environment,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        while process.poll() is None:
            sample_processes(process.pid, topology, samples)
            seen_pids.update(
                int(pid)
                for processes in samples.values()
                for pid in processes
                if pid.isdigit()
            )
            if time.monotonic() - started > timeout_seconds + BUILD_AND_CLEANUP_MARGIN_SECONDS:
                timed_out = True
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                break
            time.sleep(SAMPLE_INTERVAL_SECONDS)
        if timed_out:
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
        else:
            process.wait()
        sample_processes(process.pid, topology, samples)
    elapsed_ms = round((time.monotonic() - started) * 1_000)
    stdout_text = stdout_path.read_text(encoding="utf-8", errors="replace")
    stderr_text = stderr_path.read_text(encoding="utf-8", errors="replace")
    marker = ROUTED_SUCCESS_MARKER if topology == "routed" else DIRECT_SUCCESS_MARKER
    failures: list[str] = []
    if process.returncode != 0:
        failures.append(f"{topology} launcher exited with status {process.returncode}")
    if timed_out:
        failures.append(f"{topology} launcher exceeded bounded watchdog")
    if marker not in stdout_text and marker not in stderr_text:
        failures.append(f"{topology} success marker was not observed")
    leftovers = terminate_owned_processes(seen_pids)
    if leftovers:
        failures.append(f"{topology} Brawler descendants remained: {leftovers}")
    cpu = enforce_cpu_role_cardinality(process_cpu_summary(samples), topology)
    return {
        "pair": pair,
        "topology": topology,
        "mode": mode,
        "status": "pass" if not failures else "fail",
        "elapsed_ms": elapsed_ms,
        "exit_status": process.returncode,
        "timed_out": timed_out,
        "success_marker": marker in stdout_text or marker in stderr_text,
        "cpu": cpu,
        "rss": rss_summary(samples),
        "sample_counts": sample_counts(samples),
        "threshold_failures": failures,
        "artifacts": {"stdout": str(stdout_path), "stderr": str(stderr_path)},
        "environment_contract": {
            key: value
            for key, value in sorted(env_overrides.items())
            if key
            not in {
                "BRAWLER_ROUTED_METRICS_FILE",
                "BRAWLER_DIAGNOSTICS_DIR",
                "BRAWLER_NETWORK_ADDR",
                "BRAWLER_ROUTED_BIND",
            }
        },
    }


def read_direct_bandwidth(run: dict[str, Any]) -> dict[str, Any]:
    diagnostics = Path(run["artifacts"]["stderr"]).parent / "diagnostics"
    server_path = diagnostics / "server.closeout"
    fields, error = parse_closeout(server_path)
    result: dict[str, Any] = {
        "status": "unsupported",
        "scope": "direct server Lightyear transport counters",
        "closeout": str(server_path),
        "parse_error": error,
    }
    if fields is None:
        return result
    received = numeric_field(fields, "transport_bytes_received")
    sent = numeric_field(fields, "transport_bytes_sent")
    packets_received = numeric_field(fields, "packets_received")
    packets_sent = numeric_field(fields, "packets_sent")
    result.update(
        {
            "ingress_bytes": received,
            "egress_bytes": sent,
            "ingress_packets": packets_received,
            "egress_packets": packets_sent,
            "status": "measured" if received is not None and sent is not None else "unsupported",
            "manifest": {key: fields.get(key) for key in ("scenario_id", "run_id", "mode", "rules_profile")},
        }
    )
    return result


def read_routed_bandwidth(run: dict[str, Any]) -> dict[str, Any]:
    metrics_path = Path(run["artifacts"]["metrics"])
    try:
        metrics = json.loads(metrics_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {
            "status": "unsupported",
            "scope": "routed supervisor match-worker inner boundary counters",
            "metrics": str(metrics_path),
            "parse_error": str(error),
        }
    traffic = metrics.get("traffic")
    if not isinstance(traffic, dict):
        return {
            "status": "unsupported",
            "scope": "routed supervisor match-worker inner boundary counters",
            "metrics": str(metrics_path),
            "parse_error": "metrics.traffic is not an object",
        }

    def counter(direction: str) -> dict[str, int] | None:
        value = traffic.get(direction)
        if not isinstance(value, dict):
            return None
        values = {key: value.get(key) for key in ("bytes", "datagrams", "frames")}
        if not all(isinstance(item, int) and item >= 0 for item in values.values()):
            return None
        return {key: int(item) for key, item in values.items()}

    inner_ingress = counter("inner_ingress")
    inner_egress = counter("inner_egress")
    match_inner_ingress = counter("match_inner_ingress")
    match_inner_egress = counter("match_inner_egress")
    public_ingress = counter("public_ingress")
    public_egress = counter("public_egress")
    ipc_to_worker = counter("ipc_to_worker")
    ipc_from_worker = counter("ipc_from_worker")
    result: dict[str, Any] = {
        "status": (
            "measured" if match_inner_ingress and match_inner_egress else "unsupported"
        ),
        "scope": "routed supervisor match-worker inner boundary counters",
        "metrics": str(metrics_path),
        "ingress_bytes": match_inner_ingress["bytes"] if match_inner_ingress else None,
        "egress_bytes": match_inner_egress["bytes"] if match_inner_egress else None,
        "ingress_packets": (
            match_inner_ingress["datagrams"] if match_inner_ingress else None
        ),
        "egress_packets": match_inner_egress["datagrams"] if match_inner_egress else None,
        "match_inner": {
            "ingress": match_inner_ingress,
            "egress": match_inner_egress,
        },
        "all_inner": {"ingress": inner_ingress, "egress": inner_egress},
        "overhead": {},
    }
    for direction, public, inner in (
        ("ingress", public_ingress, inner_ingress),
        ("egress", public_egress, inner_egress),
    ):
        if public is None or inner is None:
            result["overhead"][direction] = {"status": "unsupported"}
            continue
        expected = inner["bytes"] + PUBLIC_ENVELOPE_BYTES * public["datagrams"]
        result["overhead"][direction] = {
            "status": "pass" if expected == public["bytes"] else "fail",
            "public_bytes": public["bytes"],
            "inner_bytes": inner["bytes"],
            "public_datagrams": public["datagrams"],
            "expected_public_bytes": expected,
            "envelope_overhead_bytes": public["bytes"] - inner["bytes"],
            "envelope_overhead_bytes_per_datagram": PUBLIC_ENVELOPE_BYTES,
        }
    result["overhead"]["ipc"] = {
        "status": "measured_mixed_control_and_packet_frames"
        if ipc_to_worker is not None and ipc_from_worker is not None
        else "unsupported",
        "to_worker": ipc_to_worker,
        "from_worker": ipc_from_worker,
        "bytes_total": (
            (ipc_to_worker["bytes"] + ipc_from_worker["bytes"])
            if ipc_to_worker is not None and ipc_from_worker is not None
            else None
        ),
        "comparison": "not compared with direct gameplay bytes",
    }
    return result


def compare_directional_bandwidth(direct: dict[str, Any], routed: dict[str, Any]) -> dict[str, Any]:
    directions: dict[str, Any] = {}
    failures: list[str] = []
    for direction in ("ingress", "egress"):
        direct_bytes = direct.get(f"{direction}_bytes")
        routed_bytes = routed.get(f"{direction}_bytes")
        check: dict[str, Any] = {
            "direct_bytes": direct_bytes,
            "routed_inner_bytes": routed_bytes,
            "limit_ratio": BANDWIDTH_REGRESSION_LIMIT,
            "status": "unsupported",
        }
        if isinstance(direct_bytes, int) and isinstance(routed_bytes, int) and direct_bytes > 0:
            ratio = (routed_bytes - direct_bytes) / direct_bytes
            check.update({"regression_ratio": ratio, "status": "pass" if ratio <= BANDWIDTH_REGRESSION_LIMIT else "fail"})
            if ratio > BANDWIDTH_REGRESSION_LIMIT:
                failures.append(
                    f"{direction} routed inner gameplay bytes regressed {ratio:.3f}, "
                    f"limit {BANDWIDTH_REGRESSION_LIMIT:.3f}"
                )
        directions[direction] = check
    direct_total = direct.get("ingress_bytes") + direct.get("egress_bytes") if all(
        isinstance(direct.get(key), int) for key in ("ingress_bytes", "egress_bytes")
    ) else None
    routed_total = routed.get("ingress_bytes") + routed.get("egress_bytes") if all(
        isinstance(routed.get(key), int) for key in ("ingress_bytes", "egress_bytes")
    ) else None
    total: dict[str, Any] = {
        "direct_bytes": direct_total,
        "routed_inner_bytes": routed_total,
        "limit_ratio": BANDWIDTH_REGRESSION_LIMIT,
        "status": "unsupported",
    }
    if isinstance(direct_total, int) and isinstance(routed_total, int) and direct_total > 0:
        ratio = (routed_total - direct_total) / direct_total
        total.update({"regression_ratio": ratio, "status": "pass" if ratio <= BANDWIDTH_REGRESSION_LIMIT else "fail"})
    statuses = [directions[direction]["status"] for direction in ("ingress", "egress")]
    status = "fail" if failures else ("pass" if all(value == "pass" for value in statuses) else "unsupported")
    return {
        "status": status,
        "directions": directions,
        "total": total,
        "failures": failures,
        "comparison_scope": "direct server transport versus routed supervisor inner Netcode bytes",
    }


def compare_cpu(direct: dict[str, Any], routed: dict[str, Any]) -> dict[str, Any]:
    direct_cpu = direct.get("cpu", {})
    routed_cpu = routed.get("cpu", {})
    result: dict[str, Any] = {
        "status": "unsupported",
        "limit_ratio": CPU_REGRESSION_LIMIT,
        "direct_aggregate_cpu_seconds": direct_cpu.get("aggregate_cpu_seconds"),
        "routed_aggregate_cpu_seconds": routed_cpu.get("aggregate_cpu_seconds"),
        "comparison_scope": "all sampled Brawler processes in each launcher process tree",
    }
    if not direct_cpu.get("comparable") or not routed_cpu.get("comparable"):
        result["reason"] = "one or more process CPU series had fewer than two samples"
        return result
    direct_total = direct_cpu.get("aggregate_cpu_seconds")
    routed_total = routed_cpu.get("aggregate_cpu_seconds")
    if not isinstance(direct_total, (int, float)) or not isinstance(routed_total, (int, float)) or direct_total <= 0:
        result["reason"] = "direct aggregate CPU time was zero or unavailable"
        return result
    ratio = (routed_total - direct_total) / direct_total
    result["regression_ratio"] = ratio
    result["status"] = "pass" if ratio <= CPU_REGRESSION_LIMIT else "fail"
    if result["status"] == "fail":
        result["reason"] = f"routed aggregate CPU regression {ratio:.3f} exceeds {CPU_REGRESSION_LIMIT:.3f}"
    return result


def pair_run(
    root: Path,
    pair: int,
    mode: str,
    timeout_seconds: int,
    artifacts: Path,
) -> dict[str, Any]:
    # Distinct ports keep each pair hermetic even if a previous process is slow to release a socket.
    direct_port = 5100 + (pair * 2)
    routed_port = direct_port + 1
    run_id = f"m01-paired-{mode}-{pair:03d}"
    direct = run_launcher(
        root,
        "direct",
        pair,
        mode,
        timeout_seconds,
        artifacts / "direct",
        {
            "BRAWLER_NETWORK_ADDR": f"127.0.0.1:{direct_port}",
            "BRAWLER_NETWORK_HEADLESS": "1",
            "BRAWLER_NETWORK_TIMEOUT_SECONDS": str(timeout_seconds),
            "BRAWLER_NETWORK_SIMULATION_TICKS": str(COMPARABLE_SIMULATION_TICKS),
            # The ordinary direct verification launcher exits as soon as its movement assertion
            # completes. Keep that already-authoritative server alive for the same fixed-tick
            # observation window as the routed match before comparing cumulative totals.
            "BRAWLER_SERVER_EXIT_AFTER_VERIFICATION_MIN_TICKS": str(
                COMPARABLE_SIMULATION_TICKS + 100
            ),
            "BRAWLER_NETWORK_GAME_MODE": mode,
            "BRAWLER_NETWORK_MATCH_RULES": "verification",
            "BRAWLER_NETWORK_SERVER_FEATURES": "server,process-metrics",
            "BRAWLER_NETWORK_CLIENT_FEATURES": "client,process-metrics",
            "BRAWLER_NETWORK_RUN_ID": run_id,
            "BRAWLER_NETWORK_PROFILE": "paired-m01-movement",
            "BRAWLER_DIAGNOSTICS_DIR": str(artifacts / "direct" / "diagnostics"),
            "BRAWLER_DIAGNOSTICS_SCENARIO_ID": run_id,
            "BRAWLER_DIAGNOSTICS_MODE": mode,
            "BRAWLER_DIAGNOSTICS_RULES_PROFILE": "verification",
        },
    )
    routed_metrics = artifacts / "routed" / "supervisor-metrics.json"
    routed_window_dir = artifacts / "routed" / "window"
    routed_window_dir.mkdir(parents=True, exist_ok=True)
    routed = run_launcher(
        root,
        "routed",
        pair,
        mode,
        timeout_seconds,
        artifacts / "routed",
        {
            "BRAWLER_ROUTED_BIND": f"127.0.0.1:{routed_port}",
            "BRAWLER_NETWORK_HEADLESS": "1",
            "BRAWLER_ROUTED_TIMEOUT_SECONDS": str(timeout_seconds),
            "BRAWLER_ROUTED_SIMULATION_TICKS": str(COMPARABLE_SIMULATION_TICKS),
            "BRAWLER_ROUTED_GAME_MODE": mode,
            "BRAWLER_ROUTED_MATCH_RULES": "verification",
            "BRAWLER_ROUTED_SERVER_FEATURES": "server,process-metrics",
            "BRAWLER_ROUTED_CLIENT_FEATURES": "client,process-metrics",
            "BRAWLER_ROUTED_METRICS_FILE": str(routed_metrics),
            "BRAWLER_ROUTED_WINDOW_DIR": str(routed_window_dir),
            "BRAWLER_NETWORK_RUN_ID": run_id,
            "BRAWLER_NETWORK_PROFILE": "paired-m01-movement",
            "BRAWLER_DIAGNOSTICS_SCENARIO_ID": run_id,
            "BRAWLER_DIAGNOSTICS_MODE": mode,
            "BRAWLER_DIAGNOSTICS_RULES_PROFILE": "verification",
        },
    )
    direct_bandwidth = read_direct_bandwidth(direct)
    routed["artifacts"]["metrics"] = str(routed_metrics)
    routed_bandwidth = read_routed_bandwidth(routed)
    direct_window = read_common_window(
        Path(direct["artifacts"]["stderr"]).parent / "diagnostics" / "server.window"
    )
    routed_window = read_common_window(routed_window_dir / "match.window")
    window = compare_common_windows(direct_window, routed_window)
    cpu_diagnostic = compare_cpu(direct, routed)
    bandwidth_diagnostic = compare_directional_bandwidth(direct_bandwidth, routed_bandwidth)
    if window["status"] == "measured":
        direct_window_bandwidth = {
            "ingress_bytes": direct_window["transport_bytes_received"],
            "egress_bytes": direct_window["transport_bytes_sent"],
        }
        routed_window_bandwidth = {
            "ingress_bytes": routed_window["transport_bytes_received"],
            "egress_bytes": routed_window["transport_bytes_sent"],
        }
        bandwidth_comparison = compare_directional_bandwidth(
            direct_window_bandwidth, routed_window_bandwidth
        )
        bandwidth_comparison["comparison_scope"] = (
            "direct-server versus routed-match-worker Lightyear transport deltas over the "
            "first authoritative Active->Completed interval"
        )
        window["bandwidth_scope"] = bandwidth_comparison["comparison_scope"]
    else:
        bandwidth_comparison = {
            "status": "unsupported",
            "reason": window.get("reason", "common authoritative window is unavailable"),
            "diagnostic_unthresholded": bandwidth_diagnostic,
        }
    cpu = {
        "status": "unsupported",
        "limit_ratio": CPU_REGRESSION_LIMIT,
        "reason": (
            window.get("reason")
            or "CPU samples are not timestamp-correlated to the common authoritative window"
        ),
        "diagnostic_unthresholded": cpu_diagnostic,
    }
    bandwidth = {
        "status": "unsupported",
        "limit_ratio": BANDWIDTH_REGRESSION_LIMIT,
        "reason": window.get("reason"),
        "diagnostic_unthresholded": bandwidth_diagnostic,
    }
    if bandwidth_comparison["status"] in {"pass", "fail"}:
        bandwidth = bandwidth_comparison
    failures = direct["threshold_failures"] + routed["threshold_failures"]
    if bandwidth.get("status") == "fail":
        failures.extend(bandwidth.get("failures", []))
    return {
        "pair": pair,
        "mode": mode,
        "status": "pass" if not failures else "fail",
        "direct": direct,
        "routed": routed,
        "cpu": cpu,
        "measurement_window": window,
        "bandwidth": {
            "direct": direct_bandwidth,
            "routed": routed_bandwidth,
            "comparison": bandwidth,
        },
        "threshold_failures": failures,
    }


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if not 1 <= args.pairs <= MAX_PAIRS:
        raise SystemExit(f"--pairs must be between 1 and {MAX_PAIRS}")
    if not 1 <= args.timeout <= 120:
        raise SystemExit("--timeout must be between 1 and 120 seconds")
    root = Path(__file__).resolve().parent.parent
    if args.output is None:
        stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        output = root / "target" / f"paired-evidence-{stamp}.json"
    else:
        output = args.output if args.output.is_absolute() else root / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    artifacts = output.parent / f"{output.stem}-artifacts"
    if output.exists() or artifacts.exists():
        raise SystemExit(
            "paired evidence refuses to reuse an existing summary or artifact directory"
        )
    artifacts.mkdir(parents=True, exist_ok=True)
    source_revision = "unknown"
    source_dirty = None
    try:
        source_revision = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--short", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        source_dirty = bool(
            subprocess.run(
                ["git", "-C", str(root), "status", "--porcelain"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
        )
    except (OSError, subprocess.CalledProcessError):
        pass

    pairs: list[dict[str, Any]] = []
    for pair in range(1, args.pairs + 1):
        result = pair_run(root, pair, args.mode, args.timeout, artifacts / f"pair-{pair:03d}")
        pairs.append(result)
        print(
            f"paired evidence mode={args.mode} pair={pair} status={result['status']} "
            f"cpu={result['cpu']['status']} bandwidth={result['bandwidth']['comparison']['status']}",
            file=sys.stderr,
        )

    cpu_measured = [pair["cpu"] for pair in pairs if pair["cpu"]["status"] in {"pass", "fail"}]
    bandwidth_measured = [
        pair["bandwidth"]["comparison"]
        for pair in pairs
        if pair["bandwidth"]["comparison"]["status"] in {"pass", "fail"}
    ]
    failures = [
        f"pair {pair['pair']}: {failure}"
        for pair in pairs
        for failure in pair["threshold_failures"]
    ]
    cpu_status = "fail" if any(item["status"] == "fail" for item in cpu_measured) else (
        "pass" if len(cpu_measured) == len(pairs) and pairs else "unsupported"
    )
    bandwidth_status = "fail" if any(item["status"] == "fail" for item in bandwidth_measured) else (
        "pass" if len(bandwidth_measured) == len(pairs) and pairs else "unsupported"
    )
    overall_status = "fail" if failures else (
        "pass" if cpu_status == "pass" and bandwidth_status == "pass" else "unsupported"
    )
    summary = {
        "schema": SCHEMA,
        "status": overall_status,
        "mode": args.mode,
        "profile": args.profile,
        "pairs_requested": args.pairs,
        "pairs_completed": len(pairs),
        "timeout_seconds": args.timeout,
        "sample_interval_seconds": SAMPLE_INTERVAL_SECONDS,
        "host": {
            "platform": platform.platform(),
            "python": platform.python_version(),
            "source_revision": source_revision,
            "source_dirty": source_dirty,
            "comparison_contract": "same host, source tree, mode, rules profile, and feature builds",
        },
        "hard_gates": {
            "aggregate_cpu": {
                "status": cpu_status,
                "limit_ratio": CPU_REGRESSION_LIMIT,
                "measured_pairs": len(cpu_measured),
                "scope": "all Brawler processes sampled in direct and routed launcher trees",
            },
            "inner_gameplay_bandwidth": {
                "status": bandwidth_status,
                "limit_ratio": BANDWIDTH_REGRESSION_LIMIT,
                "measured_pairs": len(bandwidth_measured),
                "directionality": ["ingress", "egress"],
                "scope": "direct server transport versus routed supervisor inner Netcode boundary",
            },
        },
        "routed_overhead": {
            "scope": "diagnostic only; excluded from direct gameplay comparison",
            "public_envelope_formula": "public_bytes = inner_bytes + 42 * datagrams",
            "ipc_scope": "framed IPC counters include packet and control frames",
        },
        "threshold_failures": failures,
        "pairs": pairs,
    }
    output.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2, sort_keys=True))
    print(f"paired evidence summary: {output}", file=sys.stderr)
    if overall_status == "pass":
        return 0
    if overall_status == "unsupported":
        return 2
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
