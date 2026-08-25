#!/usr/bin/env python3
"""Bounded, local v2 M01 routed-process evidence.

The harness deliberately measures only facts available from the existing supervisor lifecycle
logs, its post-shutdown counter snapshot, and ``ps``. It does not claim packet latency, CPU,
bandwidth, or MTU-capture gates that require a paired packet capture or a separately instrumented
benchmark. A non-zero exit means an observed supported threshold failed.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import os
from pathlib import Path
import re
import signal
import shlex
import subprocess
import sys
import time
from typing import Any


SCHEMA = "brawler-routed-evidence-v1"
READY_HARD_LIMIT_MS = 5_000
READY_P95_TARGET_MS = 2_000
HANDOFF_HARD_LIMIT_MS = 8_000
HANDOFF_P95_TARGET_MS = 3_000
GRACEFUL_STOP_LIMIT_MS = 2_000
FORCED_STOP_TOTAL_LIMIT_MS = 3_000
# The canonical launcher performs locked role builds before starting its own runtime watchdog.
# Keep the evidence owner's outer deadline bounded while allowing an uncached local role rebuild;
# process lifecycle thresholds are derived from supervisor timestamps, never from this margin.
BUILD_AND_CLEANUP_MARGIN_SECONDS = 75
RSS_LIMITS_KIB = {
    "supervisor": 32 * 1024,
    "lobby": 45 * 1024,
    "match": 50 * 1024,
}
# A successful Result revokes both match capabilities before the clients have observed teardown.
# Their already-in-flight Netcode datagrams are expected to fail closed as `Revoked`; keep that
# terminal race bounded instead of treating a security rejection as a transport/queue failure.
MAX_TERMINAL_REVOKED_DATAGRAMS_PER_CAPABILITY = 16
ROUTED_SUCCESS_MARKER = "brawler routed network: two-client lobby-to-match-to-fresh-lobby transition passed"

UNSUPPORTED_GATES = [
    {
        "name": "cpu_vs_direct_baseline",
        "status": "unsupported",
        "reason": "this harness does not claim a paired process-time baseline",
    },
    {
        "name": "bandwidth_vs_direct_baseline",
        "status": "unsupported",
        "reason": "directional counters are emitted, but this harness has no paired direct-UDP baseline",
    },
    {
        "name": "ipv4_ipv6_mtu_capture",
        "status": "unsupported",
        "reason": "packet capture and a dual-stack path fixture are outside this local harness",
    },
    {
        "name": "fixed_tick_regression",
        "status": "unsupported",
        "reason": "fixed-tick paired performance tests remain a separate command",
    },
    {
        "name": "public_receive_to_worker_decode_latency",
        "status": "unsupported",
        "reason": (
            "the supervisor timestamps stop at packet-IPC enqueue; worker decode and the next "
            "Bevy schedule are not instrumented"
        ),
    },
    {
        "name": "worker_send_to_public_receive_latency",
        "status": "unsupported",
        "reason": (
            "the supervisor timestamp starts after BRPK decode and ends at UDP send; public "
            "receive and client delivery are not instrumented"
        ),
    },
    {
        "name": "ipc_packet_exact_overhead",
        "status": "unsupported",
        "reason": (
            "IPC counters combine packet and control streams; packet payload bytes are not "
            "reported separately"
        ),
    },
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run bounded routed lobby-to-match process evidence cycles."
    )
    parser.add_argument(
        "--cycles",
        type=int,
        default=int(os.environ.get("BRAWLER_ROUTED_EVIDENCE_CYCLES", "5")),
        help="number of cold process cycles per selected gameplay mode (1..25; default: 5)",
    )
    parser.add_argument(
        "--mode",
        choices=("wipeout", "hot-zone", "heist", "both", "all", "crash-restart"),
        default=os.environ.get("BRAWLER_ROUTED_EVIDENCE_MODE", "wipeout"),
        help=(
            "evidence profile: one gameplay mode, both legacy modes, all modes, or the "
            "production-worker crash/restart process test (default: wipeout)"
        ),
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=int(os.environ.get("BRAWLER_ROUTED_EVIDENCE_TIMEOUT_SECONDS", "90")),
        help="per-cycle routed smoke timeout in seconds (1..120; default: 90)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="summary JSON path (default: target/routed-evidence-<UTC timestamp>.json)",
    )
    parser.add_argument(
        "--keep-artifacts",
        action="store_true",
        help="retain per-cycle stdout/stderr and ps snapshots next to the summary",
    )
    return parser.parse_args()


def ps_rows() -> list[tuple[int, int, int, str]]:
    """Read the portable RSS/process view used by the local macOS/Linux smoke."""

    try:
        result = subprocess.run(
            ["ps", "-axo", "pid=,ppid=,rss=,command="],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return []
    rows: list[tuple[int, int, int, str]] = []
    for line in result.stdout.splitlines():
        fields = line.strip().split(maxsplit=3)
        if len(fields) < 4:
            continue
        try:
            rows.append((int(fields[0]), int(fields[1]), int(fields[2]), fields[3]))
        except ValueError:
            continue
    return rows


def process_tree(root_pid: int, rows: list[tuple[int, int, int, str]]) -> set[int]:
    children: dict[int, list[int]] = {}
    for pid, ppid, _rss, _command in rows:
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


def process_role(command: str) -> str | None:
    """Classify only a process whose executable is a Brawler binary.

    ``ps command=`` includes the complete argv.  Looking for a binary name anywhere in that
    string mistakes build commands, shell wrappers, and this evidence harness itself for live
    workers (for example, ``cargo build --bin brawler-supervisor``).  The first argv token is the
    executable reported by the portable ``ps`` view; role flags are inspected only after that
    boundary.
    """

    try:
        argv = shlex.split(command)
    except ValueError:
        return None
    if not argv:
        return None
    executable = Path(argv[0]).name.lower()
    args = [argument.lower() for argument in argv[1:]]
    if executable == "brawler-supervisor":
        return "supervisor"
    if executable != "brawler-server":
        return None
    role = None
    for index, argument in enumerate(args):
        if argument == "--role" and index + 1 < len(args):
            role = args[index + 1]
            break
        if argument.startswith("--role="):
            role = argument.partition("=")[2]
            break
    if role == "lobby":
        return "lobby"
    if role == "match":
        return "match"
    return None


def sample_rss(
    root_pid: int,
    samples: dict[str, dict[str, dict[str, int]]],
    seen_pids: set[int],
) -> None:
    rows = ps_rows()
    row_by_pid = {pid: (rss, command) for pid, _ppid, rss, command in rows}
    for pid in process_tree(root_pid, rows):
        entry = row_by_pid.get(pid)
        if entry is None:
            continue
        rss, command = entry
        role = process_role(command)
        if role is None:
            continue
        seen_pids.add(pid)
        process = samples.setdefault(role, {}).setdefault(
            str(pid), {"samples": 0, "max_rss_kib": 0}
        )
        process["samples"] += 1
        process["max_rss_kib"] = max(process["max_rss_kib"], rss)


def terminate_owned_processes(pids: set[int]) -> list[int]:
    """Terminate only still-live processes previously identified as Brawler descendants."""

    live: list[int] = []
    rows = {pid: command for pid, _ppid, _rss, command in ps_rows()}
    for pid in sorted(pids):
        if pid not in rows or process_role(rows[pid]) is None:
            continue
        try:
            os.kill(pid, signal.SIGTERM)
            live.append(pid)
        except ProcessLookupError:
            continue
        except PermissionError:
            live.append(pid)
    return live


def p95(values: list[int]) -> int | None:
    if not values:
        return None
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * 0.95) - 1)]


def parse_lifecycle(stderr: str) -> dict[str, Any]:
    # The long-lived lobby is started before the runtime begins reporting lifecycle events, so it
    # has a Ready record but no Spawned record in the supervisor log.  Every worker that is
    # dynamically spawned during this smoke is therefore a match worker.  Keep the identity sets
    # so a persistent lobby is not incorrectly required to be reaped during the match cycle.
    # Keep ordered events rather than independently counting regex matches.  A count-only parser
    # can accidentally accept a partial worker lifecycle (or a Result from another worker) while
    # still reporting one of each event.  The production supervisor currently logs a successful
    # terminal match in this order: spawned, ready, result-received, reaped, stopped, cleaned.
    event_patterns = (
        (
            "spawned",
            re.compile(
                r"worker spawned worker=(?P<worker>\d+) pid=(?P<pid>\d+) "
                r"elapsed_ms=(?P<elapsed>\d+)"
            ),
        ),
        (
            "ready",
            re.compile(
                r"worker ready worker=(?P<worker>\d+) elapsed_ms=(?P<elapsed>\d+)"
            ),
        ),
        (
            "result_received",
            re.compile(
                r"worker result-received worker=(?P<worker>\d+)"
                r"(?: elapsed_ms=(?P<elapsed>\d+))?(?:\s|$)"
            ),
        ),
        (
            "stop_requested",
            re.compile(
                r"worker stop-requested worker=(?P<worker>\d+)"
                r" stop_id=(?P<stop_id>\d+) elapsed_ms=(?P<elapsed>\d+)"
                r"(?: ts_ms=(?P<timestamp_ms>\d+))?"
            ),
        ),
        (
            "stop_sent",
            re.compile(
                r"worker stop-sent worker=(?P<worker>\d+)"
                r"(?: stop_id=(?P<stop_id>\d+))?"
                r"(?: elapsed_ms=(?P<elapsed>\d+))?"
                r"(?: ts_ms=(?P<timestamp_ms>\d+))?(?:\s|$)"
            ),
        ),
        (
            "reaped",
            re.compile(
                r"worker reaped worker=(?P<worker>\d+) success=(?P<success>true|false) "
                r"code=(?P<code>\S+) elapsed_ms=(?P<elapsed>\d+)"
            ),
        ),
        (
            "stopped",
            re.compile(
                r"worker stopped worker=(?P<worker>\d+) forced=(?P<forced>true|false) "
                r"elapsed_ms=(?P<elapsed>\d+)"
            ),
        ),
        (
            "forced_stop",
            re.compile(
                r"worker forced-stop worker=(?P<worker>\d+) elapsed_ms=(?P<elapsed>\d+)"
            ),
        ),
        (
            "cleaned",
            re.compile(
                r"worker cleaned worker=(?P<worker>\d+) elapsed_ms=(?P<elapsed>\d+)"
            ),
        ),
    )
    events: list[dict[str, Any]] = []
    for line in stderr.splitlines():
        for kind, pattern in event_patterns:
            match = pattern.search(line)
            if match is None:
                continue
            event: dict[str, Any] = {"kind": kind, "worker": match.group("worker")}
            for field in ("elapsed", "pid", "stop_id", "timestamp_ms"):
                if field in match.groupdict() and match.group(field) is not None:
                    event[field] = int(match.group(field))
            for field in ("success", "forced"):
                if field in match.groupdict() and match.group(field) is not None:
                    event[field] = match.group(field) == "true"
            events.append(event)
            break

    spawned_events = [event for event in events if event["kind"] == "spawned"]
    ready_events = [event for event in events if event["kind"] == "ready"]
    result_events = [event for event in events if event["kind"] == "result_received"]
    stop_requested_events = [event for event in events if event["kind"] == "stop_requested"]
    stop_sent_events = [event for event in events if event["kind"] == "stop_sent"]
    reaped_events = [event for event in events if event["kind"] == "reaped"]
    stopped_events = [event for event in events if event["kind"] == "stopped"]
    forced_stop_events = [event for event in events if event["kind"] == "forced_stop"]
    cleaned_events = [event for event in events if event["kind"] == "cleaned"]

    spawned_workers = [event["worker"] for event in spawned_events]
    match_workers = set(spawned_workers)
    ready_workers = [event["worker"] for event in ready_events]
    result_workers = [event["worker"] for event in result_events]
    stop_requested_workers = [event["worker"] for event in stop_requested_events]
    stop_sent_workers = [event["worker"] for event in stop_sent_events]
    reaped_workers = [event["worker"] for event in reaped_events]
    stopped_workers = [event["worker"] for event in stopped_events]
    forced_stop_workers = [event["worker"] for event in forced_stop_events]
    cleaned_workers = [event["worker"] for event in cleaned_events]

    # The lobby is the one worker that exists before runtime lifecycle reporting starts.  Treat its
    # Ready event as a one-shot guard: a duplicate Ready is evidence of a broken restart or an
    # ambiguous log, even when both records carry the same worker ID.
    lobby_ready_events = [worker for worker in ready_workers if worker not in match_workers]
    lobby_worker = lobby_ready_events[0] if len(lobby_ready_events) == 1 else None

    sequence_failures: list[str] = []
    if len(spawned_events) != 1 or len(match_workers) != 1:
        sequence_failures.append(
            "expected exactly one dynamic match spawn, "
            f"observed {len(spawned_events)} spawn events for workers {sorted(match_workers)}"
        )
    unknown_workers = sorted(
        {
            worker
            for worker in ready_workers
            + result_workers
            + stop_requested_workers
            + stop_sent_workers
            + reaped_workers
            + stopped_workers
            + forced_stop_workers
            + cleaned_workers
            if worker not in match_workers and worker not in lobby_ready_events
        }
    )
    if unknown_workers:
        sequence_failures.append(
            f"lifecycle event referenced non-match worker(s): {unknown_workers}"
        )
    if len(lobby_ready_events) != 1:
        sequence_failures.append(
            "expected exactly one persistent lobby Ready event, "
            f"observed {len(lobby_ready_events)}"
        )

    # The lobby is long-lived during the routed cycle, but it must still be shut down normally by
    # the supervisor before the evidence cycle is accepted.  A successful match teardown alone is
    # insufficient: accepting a crash, forced stop, or orphaned lobby would make the cycle appear
    # healthy while leaking the process that owns the public endpoint.
    lobby_events = (
        [event for event in events if event["worker"] == lobby_worker]
        if lobby_worker is not None
        else []
    )
    observed_lobby_sequence = [event["kind"] for event in lobby_events]
    expected_lobby_sequence = [
        "ready",
        "stop_requested",
        "stop_sent",
        "reaped",
        "stopped",
        "cleaned",
    ]
    lobby_reaped_events = [
        event for event in lobby_events if event["kind"] == "reaped"
    ]
    lobby_reaped = [
        event["worker"] for event in lobby_reaped_events if event.get("success", False)
    ]
    lobby_failed_reaped = [
        event["worker"] for event in lobby_reaped_events if not event.get("success", False)
    ]
    lobby_stopped_events = [
        event for event in lobby_events if event["kind"] == "stopped"
    ]
    lobby_stopped_graceful = [
        event["worker"]
        for event in lobby_stopped_events
        if not event.get("forced", True)
    ]
    lobby_forced_stop = [
        event["worker"]
        for event in lobby_events
        if event["kind"] == "forced_stop"
    ]
    lobby_cleaned = [
        event["worker"] for event in lobby_events if event["kind"] == "cleaned"
    ]
    if observed_lobby_sequence != expected_lobby_sequence:
        sequence_failures.append(
            "persistent lobby lifecycle order expected "
            f"{expected_lobby_sequence}, observed {observed_lobby_sequence}"
        )
    if lobby_failed_reaped:
        sequence_failures.append(
            f"persistent lobby worker reaped unsuccessfully: {lobby_failed_reaped}"
        )
    if lobby_forced_stop:
        sequence_failures.append(
            f"persistent lobby forced stop observed: {lobby_forced_stop}"
        )
    lobby_shutdown_ok = (
        lobby_worker is not None
        and observed_lobby_sequence == expected_lobby_sequence
        and len(lobby_reaped) == 1
        and not lobby_failed_reaped
        and len(lobby_stopped_graceful) == 1
        and not lobby_forced_stop
        and len(lobby_cleaned) == 1
    )

    match_worker = next(iter(match_workers), None)
    match_events = (
        [event for event in events if event["worker"] == match_worker]
        if match_worker is not None
        else []
    )
    observed_sequence = [event["kind"] for event in match_events]
    expected_sequence = [
        "spawned",
        "ready",
        "result_received",
        # The supervisor reports child reaping before it emits the terminal graceful-stopped
        # record because both are finalized in one poll.  Keep this exact observed order honest.
        "reaped",
        "stopped",
        "cleaned",
    ]
    expected_sequence_with_stop = [
        "spawned",
        "ready",
        "result_received",
        "stop_sent",
        "reaped",
        "stopped",
        "cleaned",
    ]
    expected_sequence_with_request_and_stop = [
        "spawned",
        "ready",
        "result_received",
        "stop_requested",
        "stop_sent",
        "reaped",
        "stopped",
        "cleaned",
    ]
    if observed_sequence not in (
        expected_sequence,
        expected_sequence_with_stop,
        expected_sequence_with_request_and_stop,
    ):
        sequence_failures.append(
            "dynamic match lifecycle order expected "
            f"{expected_sequence} (or StopSent-correlated {expected_sequence_with_stop} "
            f"or request+send-correlated {expected_sequence_with_request_and_stop}), "
            f"observed {observed_sequence}"
        )

    spawned_elapsed = {
        event["worker"]: event["elapsed"]
        for event in spawned_events
        if "elapsed" in event
    }
    ready_elapsed = {
        event["worker"]: event["elapsed"] for event in ready_events if "elapsed" in event
    }
    ready = [
        max(0, ready_elapsed[match_worker] - spawned_elapsed[match_worker])
        for match_worker in sorted(match_workers)
        if match_worker in spawned_elapsed and match_worker in ready_elapsed
    ]
    match_ready = [worker for worker in ready_workers if worker in match_workers]
    match_result_received = [worker for worker in result_workers if worker in match_workers]
    match_reaped_events = [
        event for event in reaped_events if event["worker"] in match_workers
    ]
    match_reaped = [
        event["worker"] for event in match_reaped_events if event.get("success", False)
    ]
    match_failed_reaped = [
        event["worker"] for event in match_reaped_events if not event.get("success", False)
    ]
    if match_failed_reaped:
        sequence_failures.append(
            f"dynamic match worker reaped unsuccessfully: {match_failed_reaped}"
        )
    match_stopped_graceful = [
        event["worker"]
        for event in stopped_events
        if event["worker"] in match_workers and not event.get("forced", True)
    ]
    match_forced_stop = [worker for worker in forced_stop_workers if worker in match_workers]
    match_cleaned = [worker for worker in cleaned_workers if worker in match_workers]
    stop_sent_elapsed = next(
        (
            event["elapsed"]
            for event in stop_sent_events
            if event["worker"] in match_workers and "elapsed" in event
        ),
        None,
    )
    stop_requested_elapsed = next(
        (
            event["elapsed"]
            for event in stop_requested_events
            if event["worker"] in match_workers and "elapsed" in event
        ),
        None,
    )
    reaped_elapsed = next(
        (
            event["elapsed"]
            for event in reaped_events
            if event["worker"] in match_workers and "elapsed" in event
        ),
        None,
    )
    stopped_elapsed = next(
        (
            event["elapsed"]
            for event in stopped_events
            if event["worker"] in match_workers and "elapsed" in event
        ),
        None,
    )
    forced_elapsed = next(
        (
            event["elapsed"]
            for event in forced_stop_events
            if event["worker"] in match_workers and "elapsed" in event
        ),
        None,
    )
    cleaned_elapsed = next(
        (
            event["elapsed"]
            for event in cleaned_events
            if event["worker"] in match_workers and "elapsed" in event
        ),
        None,
    )
    stop_start_elapsed = (
        stop_requested_elapsed
        if stop_requested_elapsed is not None
        else stop_sent_elapsed
    )
    if stop_start_elapsed is not None and reaped_elapsed is not None:
        graceful_stop_reap_duration_ms: int | None = max(
            0, reaped_elapsed - stop_start_elapsed
        )
        graceful_stop_reap_duration_status = "measured"
    else:
        graceful_stop_reap_duration_ms = None
        graceful_stop_reap_duration_status = "unsupported"
    if stop_start_elapsed is not None and forced_elapsed is not None and cleaned_elapsed is not None:
        forced_total_duration_ms: int | None = max(0, cleaned_elapsed - stop_start_elapsed)
        forced_total_duration_status = "measured"
    else:
        forced_total_duration_ms = None
        forced_total_duration_status = "unsupported"
    return {
        # These names are retained for the existing summary shape, but counts now refer to actual
        # dynamic match lifecycle records, including duplicates, rather than unique worker IDs.
        "spawned": len(spawned_events),
        "ready": len(match_ready),
        "ready_ms": ready,
        "ready_p95_ms": p95(ready),
        "reaped": len(match_reaped),
        "stopped_graceful": len(match_stopped_graceful),
        "forced_stop": len(match_forced_stop),
        "cleaned": len(match_cleaned),
        "lobby_ready": len(lobby_ready_events),
        "lobby_ready_events": len(lobby_ready_events),
        "lobby_worker": lobby_worker,
        "lobby_shutdown_status": "pass" if lobby_shutdown_ok else "fail",
        "lobby_reaped": len(lobby_reaped),
        "lobby_failed_reaped": lobby_failed_reaped,
        "lobby_stopped_graceful": len(lobby_stopped_graceful),
        "lobby_forced_stop": len(lobby_forced_stop),
        "lobby_cleaned": len(lobby_cleaned),
        "observed_lobby_sequence": observed_lobby_sequence,
        "match_worker": match_worker,
        "match_spawned": len(spawned_events),
        "match_ready": len(match_ready),
        "match_result_received": len(match_result_received),
        "match_reaped": len(match_reaped),
        "match_reaped_events": len(match_reaped_events),
        "match_failed_reaped": match_failed_reaped,
        "match_stopped_graceful": len(match_stopped_graceful),
        "match_forced_stop": len(match_forced_stop),
        "match_cleaned": len(match_cleaned),
        "dynamic_match_workers": sorted(match_workers),
        "ready_workers": ready_workers,
        "result_workers": result_workers,
        "stop_requested_workers": stop_requested_workers,
        "stop_sent_workers": stop_sent_workers,
        "reaped_workers": reaped_workers,
        "stopped_workers": stopped_workers,
        "cleaned_workers": cleaned_workers,
        "observed_match_sequence": observed_sequence,
        "graceful_stop_reap_duration_ms": graceful_stop_reap_duration_ms,
        "graceful_stop_reap_duration_status": graceful_stop_reap_duration_status,
        "graceful_stop_cleanup_duration_ms": (
            max(0, cleaned_elapsed - stop_start_elapsed)
            if stop_start_elapsed is not None and cleaned_elapsed is not None
            else None
        ),
        "stop_requested_elapsed_ms": stop_requested_elapsed,
        "stop_sent_elapsed_ms": stop_sent_elapsed,
        "reaped_elapsed_ms": reaped_elapsed,
        "forced_total_duration_ms": forced_total_duration_ms,
        "forced_total_duration_status": forced_total_duration_status,
        "sequence_failures": sequence_failures,
    }


def parse_handoff_timing(stderr: str) -> dict[str, Any]:
    """Correlate supervisor allocation acceptance with both fresh match connections.

    Supervisor and client markers use Unix epoch milliseconds because they are emitted by separate
    processes. Only ``RequestId`` and the supervisor ``WorkerId`` are used for correlation; the
    parser rejects marker lines that attempt to include capabilities or player manifests.
    """

    allocation_pattern = re.compile(
        r"brawler-supervisor timing allocation-accepted"
        r" request_id=(?P<request_id>\d+) worker=(?P<worker>\d+)"
        r" ts_ms=(?P<timestamp_ms>\d+)"
    )
    connected_pattern = re.compile(
        r"brawler-client timing handoff-connected"
        r" client_id=(?P<client_id>\d+) request_id=(?P<request_id>\d+)"
        r" ts_ms=(?P<timestamp_ms>\d+)"
    )
    allocations: dict[str, dict[str, Any]] = {}
    connections: dict[str, list[dict[str, Any]]] = {}
    failures: list[str] = []
    redaction_failures: list[str] = []
    for line_number, line in enumerate(stderr.splitlines(), 1):
        if "timing allocation-accepted" in line or "timing handoff-connected" in line:
            lowered = line.lower()
            if any(
                field in lowered
                for field in ("capability=", "manifest=", "player_manifest=", "players=")
            ):
                redaction_failures.append(
                    f"line {line_number}: timing marker contains secret-bearing field"
                )
        allocation = allocation_pattern.search(line)
        if allocation is not None:
            event = {
                "request_id": allocation.group("request_id"),
                "worker_id": allocation.group("worker"),
                "timestamp_ms": int(allocation.group("timestamp_ms")),
                "line": line_number,
            }
            request_id = event["request_id"]
            if request_id in allocations:
                failures.append(f"duplicate allocation-accepted marker for request {request_id}")
            else:
                allocations[request_id] = event
            continue
        connected = connected_pattern.search(line)
        if connected is not None:
            event = {
                "client_id": connected.group("client_id"),
                "request_id": connected.group("request_id"),
                "timestamp_ms": int(connected.group("timestamp_ms")),
                "line": line_number,
            }
            connections.setdefault(event["request_id"], []).append(event)

    samples: list[dict[str, Any]] = []
    for request_id, allocation in allocations.items():
        connected = connections.get(request_id, [])
        client_ids = [event["client_id"] for event in connected]
        if len(connected) != 2 or len(set(client_ids)) != 2:
            failures.append(
                f"request {request_id} expected two distinct match connections, observed {client_ids}"
            )
            continue
        durations = []
        for event in connected:
            duration = event["timestamp_ms"] - allocation["timestamp_ms"]
            if duration < 0:
                failures.append(
                    f"request {request_id} client {event['client_id']} connected before allocation"
                )
            durations.append(max(0, duration))
        handoff_ms = max(durations)
        samples.append(
            {
                "request_id": request_id,
                "worker_id": allocation["worker_id"],
                "allocation_timestamp_ms": allocation["timestamp_ms"],
                "connected_clients": sorted(client_ids),
                "connected_timestamp_ms": {
                    event["client_id"]: event["timestamp_ms"] for event in connected
                },
                "handoff_ms": handoff_ms,
            }
        )

    unknown_requests = sorted(set(connections) - set(allocations))
    failures.extend(
        f"handoff connection marker referenced unknown request {request_id}"
        for request_id in unknown_requests
    )
    failures.extend(redaction_failures)
    handoff_values = [sample["handoff_ms"] for sample in samples]
    if any(value > HANDOFF_HARD_LIMIT_MS for value in handoff_values):
        failures.extend(
            f"allocation-to-connected exceeded {HANDOFF_HARD_LIMIT_MS}ms: {value}ms"
            for value in handoff_values
            if value > HANDOFF_HARD_LIMIT_MS
        )
    return {
        "status": "measured" if samples and not failures else ("invalid" if failures else "unsupported"),
        "sample_count": len(samples),
        "handoff_ms": handoff_values,
        "p95_ms": p95(handoff_values),
        "samples": samples,
        "failures": failures,
        "redaction_status": "pass" if not redaction_failures else "fail",
    }


def metrics_failures(metrics: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    for key in (
        "workers",
        "routes",
        "process_workers",
        "packet_current_frames",
        "packet_current_bytes",
        "control_current_frames",
        "control_current_bytes",
        "packet_dropped_newest",
        "control_rejected",
        "source_limited",
        "live_capabilities",
    ):
        if metrics.get(key) != 0:
            failures.append(f"metrics.{key} expected 0, observed {metrics.get(key)!r}")
    if metrics.get("runtime_dir_entries") != 0:
        failures.append(
            "metrics.runtime_dir_entries expected 0, "
            f"observed {metrics.get('runtime_dir_entries')!r}"
        )
    errors = metrics.get("errors", {})
    if not isinstance(errors, dict):
        failures.append("metrics.errors is not an object")
    else:
        unexpected = dict(errors)
        revoked = unexpected.pop("Revoked", 0)
        revoked_capabilities = metrics.get("capabilities_revoked", 0)
        if not isinstance(revoked, int) or revoked < 0:
            failures.append(f"metrics.errors.Revoked is invalid: {revoked!r}")
        elif not isinstance(revoked_capabilities, int) or revoked_capabilities < 0:
            failures.append(
                "metrics.capabilities_revoked is invalid: "
                f"{revoked_capabilities!r}"
            )
        elif revoked > (
            revoked_capabilities * MAX_TERMINAL_REVOKED_DATAGRAMS_PER_CAPABILITY
        ):
            failures.append(
                "metrics.errors.Revoked exceeded the bounded terminal allowance: "
                f"{revoked} for {revoked_capabilities} revoked capabilities"
            )
        if unexpected:
            failures.append(
                f"metrics.errors contained unexpected categories: {unexpected!r}"
            )
    return failures


PUBLIC_ENVELOPE_OVERHEAD_BYTES = 42


def validate_public_traffic(traffic: dict[str, Any]) -> dict[str, Any]:
    """Validate the exact public envelope byte formula in each direction.

    The supervisor counts malformed public datagrams before decoding them, so the formula is only
    valid when public and inner datagram counts match.  A mismatch is a real evidence failure,
    not a reason to silently call the accounting approximate.
    """

    checks: dict[str, Any] = {}
    failures: list[str] = []
    for direction in ("ingress", "egress"):
        public = traffic.get(f"public_{direction}")
        inner = traffic.get(f"inner_{direction}")
        check: dict[str, Any] = {"status": "unsupported"}
        if isinstance(public, dict) and isinstance(inner, dict):
            public_datagrams = public.get("datagrams")
            inner_datagrams = inner.get("datagrams")
            public_bytes = public.get("bytes")
            inner_bytes = inner.get("bytes")
            if all(
                isinstance(value, int) and value >= 0
                for value in (public_datagrams, inner_datagrams, public_bytes, inner_bytes)
            ):
                expected_bytes = inner_bytes + PUBLIC_ENVELOPE_OVERHEAD_BYTES * public_datagrams
                datagram_match = public_datagrams == inner_datagrams
                bytes_match = public_bytes == expected_bytes
                check = {
                    "status": "pass" if datagram_match and bytes_match else "fail",
                    "public_datagrams": public_datagrams,
                    "inner_datagrams": inner_datagrams,
                    "public_bytes": public_bytes,
                    "inner_bytes": inner_bytes,
                    "expected_public_bytes": expected_bytes,
                    "datagram_counts_match": datagram_match,
                    "bytes_match": bytes_match,
                    "overhead_bytes_per_datagram": PUBLIC_ENVELOPE_OVERHEAD_BYTES,
                }
                if not datagram_match:
                    failures.append(
                        f"traffic.public_{direction}.datagrams does not match "
                        f"traffic.inner_{direction}.datagrams"
                    )
                if not bytes_match:
                    failures.append(
                        f"traffic.public_{direction}.bytes expected inner bytes + "
                        f"{PUBLIC_ENVELOPE_OVERHEAD_BYTES}*datagrams "
                        f"({expected_bytes}), observed {public_bytes}"
                    )
        checks[direction] = check
    statuses = [check["status"] for check in checks.values()]
    if failures:
        status = "fail"
    elif all(value == "pass" for value in statuses):
        status = "pass"
    else:
        status = "unsupported"
    return {
        "status": status,
        "formula": "public_bytes = inner_bytes + 42 * datagrams",
        "checks": checks,
        "failures": failures,
        "ipc_exact_overhead_status": "unsupported",
        "ipc_exact_overhead_reason": (
            "IPC counters combine packet and control streams; packet payload bytes are not "
            "reported separately"
        ),
    }


def owner_boundary_measurement(metrics: dict[str, Any]) -> dict[str, Any] | None:
    """Summarize supervisor owner-boundary processing/queue telemetry only.

    The first interval starts after ``recv_from`` returns and ends after packet-IPC enqueue.  The
    second starts after BRPK decode and ends after the supervisor's UDP send.  Neither interval
    includes worker decode, the next Bevy schedule, client delivery, or full public receive-to-
    worker-decode / worker-send-to-client latency.  Values are diagnostic stage timings, not a
    2 ms hard-gate result.
    """

    traffic = metrics.get("traffic")
    latency = metrics.get("latency")
    traffic_keys = (
        "public_ingress",
        "public_egress",
        "inner_ingress",
        "inner_egress",
        "ipc_to_worker",
        "ipc_from_worker",
    )
    latency_keys = (
        "public_receive_to_packet_ipc_enqueue",
        "worker_packet_to_public_send",
    )
    if not isinstance(traffic, dict) or not isinstance(latency, dict):
        return None
    if any(not isinstance(traffic.get(key), dict) for key in traffic_keys):
        return None
    if any(not isinstance(latency.get(key), dict) for key in latency_keys):
        return None

    def count(stage: str) -> int:
        value = latency[stage].get("samples")
        return value if isinstance(value, int) and value >= 0 else -1

    def p95_value(stage: str) -> int | None:
        value = latency[stage].get("p95_us")
        return value if isinstance(value, int) and value >= 0 else None

    public_samples = count(latency_keys[0])
    worker_samples = count(latency_keys[1])
    if public_samples < 0 or worker_samples < 0:
        return None
    public_p95 = p95_value(latency_keys[0])
    worker_p95 = p95_value(latency_keys[1])
    observed_p95 = [value for value in (public_p95, worker_p95) if value is not None]
    sample_count = min(public_samples, worker_samples)
    public_traffic = validate_public_traffic(traffic)
    return {
        "status": "measured_diagnostic",
        "scope": (
            "supervisor owner-boundary processing/queue intervals; not end-to-end and not "
            "paired to direct UDP"
        ),
        "public_receive_to_packet_ipc_enqueue": {
            "sample_count": public_samples,
            "p95_us": public_p95,
        },
        "worker_packet_to_public_send": {
            "sample_count": worker_samples,
            "p95_us": worker_p95,
        },
        "paired_sample_count": sample_count,
        "max_observed_cycle_p95_us": max(observed_p95, default=None),
        "threshold_status": "diagnostic_only",
        "public_traffic_validation": public_traffic,
        "traffic": traffic,
    }


def run_cycle(
    root: Path, cycle: int, mode: str, timeout_seconds: int, artifacts: Path
) -> dict[str, Any]:
    cycle_dir = artifacts / f"cycle-{cycle:03d}"
    cycle_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = cycle_dir / "stdout.log"
    stderr_path = cycle_dir / "stderr.log"
    metrics_path = cycle_dir / "supervisor-metrics.json"
    bind = f"127.0.0.1:{5100 + cycle}"
    env = os.environ.copy()
    env.update(
        {
            "BRAWLER_NETWORK_HEADLESS": "1",
            "BRAWLER_ROUTED_BIND": bind,
            "BRAWLER_ROUTED_GAME_MODE": mode,
            "BRAWLER_ROUTED_MATCH_RULES": "verification",
            "BRAWLER_ROUTED_TIMEOUT_SECONDS": str(timeout_seconds),
            "BRAWLER_ROUTED_METRICS_FILE": str(metrics_path),
        }
    )
    started = time.monotonic()
    samples: dict[str, dict[str, dict[str, int]]] = {}
    seen_pids: set[int] = set()
    hard_deadline = timeout_seconds + BUILD_AND_CLEANUP_MARGIN_SECONDS
    timed_out = False
    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
        "w", encoding="utf-8"
    ) as stderr:
        process = subprocess.Popen(
            ["sh", str(root / "scripts/network-routed.sh")],
            cwd=root,
            env=env,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        while process.poll() is None:
            sample_rss(process.pid, samples, seen_pids)
            if time.monotonic() - started > hard_deadline:
                timed_out = True
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                break
            time.sleep(0.05)
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
        sample_rss(process.pid, samples, seen_pids)
    elapsed_ms = round((time.monotonic() - started) * 1_000)
    stdout_text = stdout_path.read_text(encoding="utf-8", errors="replace")
    stderr_text = stderr_path.read_text(encoding="utf-8", errors="replace")
    lifecycle = parse_lifecycle(stderr_text)
    handoff = parse_handoff_timing(stderr_text)
    metrics: dict[str, Any] | None = None
    metrics_error: str | None = None
    try:
        metrics = json.loads(metrics_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        metrics_error = str(error)
    failures: list[str] = []
    if process.returncode != 0:
        failures.append(f"network-routed.sh exited with status {process.returncode}")
    if timed_out:
        failures.append(f"cycle exceeded bounded {hard_deadline}s watchdog")
    stdout_marker = ROUTED_SUCCESS_MARKER in stdout_text
    if not stdout_marker:
        failures.append("routed fresh-lobby success marker was not observed")
    failures.extend(f"lifecycle: {failure}" for failure in lifecycle["sequence_failures"])
    failures.extend(f"handoff: {failure}" for failure in handoff["failures"])
    if lifecycle["lobby_ready"] != 1:
        failures.append(
            "expected exactly one persistent lobby worker Ready event, "
            f"observed {lifecycle['lobby_ready']}"
        )
    if lifecycle["match_spawned"] != 1:
        failures.append(
            "expected exactly 1 dynamically spawned match worker, "
            f"observed {lifecycle['match_spawned']}"
        )
    if lifecycle["match_ready"] != 1:
        failures.append(
            "expected the one dynamically spawned match worker to become ready exactly once, "
            f"observed {lifecycle['match_ready']}"
        )
    if lifecycle["match_result_received"] != 1:
        failures.append(
            "expected exactly one ResultReceived from the dynamic match worker, "
            f"observed {lifecycle['match_result_received']}"
        )
    if lifecycle["ready_ms"] and max(lifecycle["ready_ms"]) > READY_HARD_LIMIT_MS:
        failures.append(
            f"spawn-to-ready exceeded {READY_HARD_LIMIT_MS}ms: {lifecycle['ready_ms']}"
        )
    if lifecycle["match_reaped"] != 1:
        failures.append(
            "expected the one match worker to be reaped successfully exactly once, "
            f"observed {lifecycle['match_reaped']}"
        )
    if lifecycle["match_stopped_graceful"] != 1:
        failures.append(
            "expected the one match worker to stop gracefully exactly once, "
            f"observed {lifecycle['match_stopped_graceful']}"
        )
    if lifecycle["forced_stop"]:
        failures.append(f"forced stop observed: {lifecycle['forced_stop']}")
    if lifecycle["graceful_stop_reap_duration_status"] != "measured":
        failures.append("graceful stop/reap timing markers were not correlated")
    if (
        lifecycle["graceful_stop_reap_duration_ms"] is not None
        and lifecycle["graceful_stop_reap_duration_ms"] > GRACEFUL_STOP_LIMIT_MS
    ):
        failures.append(
            f"graceful stop/reap duration exceeded {GRACEFUL_STOP_LIMIT_MS}ms: "
            f"{lifecycle['graceful_stop_reap_duration_ms']}ms"
        )
    if (
        lifecycle["forced_total_duration_status"] == "measured"
        and lifecycle["forced_total_duration_ms"] is not None
        and lifecycle["forced_total_duration_ms"] > FORCED_STOP_TOTAL_LIMIT_MS
    ):
        failures.append(
            f"forced stop total duration exceeded {FORCED_STOP_TOTAL_LIMIT_MS}ms: "
            f"{lifecycle['forced_total_duration_ms']}ms"
        )
    if metrics is None:
        failures.append(f"supervisor metrics unavailable: {metrics_error}")
    else:
        failures.extend(metrics_failures(metrics))
    routing = owner_boundary_measurement(metrics) if metrics is not None else None
    if metrics is not None and routing is None:
        failures.append("supervisor routing telemetry unavailable or malformed")
    elif routing is not None:
        failures.extend(
            f"traffic: {failure}"
            for failure in routing["public_traffic_validation"]["failures"]
        )
    rss_max: dict[str, int | None] = {}
    for role, limit in RSS_LIMITS_KIB.items():
        role_samples = samples.get(role, {})
        maximum = max(
            (entry["max_rss_kib"] for entry in role_samples.values()), default=None
        )
        rss_max[role] = maximum
        if maximum is None:
            failures.append(f"rss.{role} was not observed")
        elif maximum > limit:
            failures.append(
                f"rss.{role} exceeded {limit}KiB: observed {maximum}KiB"
            )
    leftover = terminate_owned_processes(seen_pids)
    if leftover:
        failures.append(f"Brawler descendants remained after cycle: {leftover}")
    return {
        "cycle": cycle,
        "mode": mode,
        "bind": bind,
        "status": "pass" if not failures else "fail",
        "elapsed_ms": elapsed_ms,
        "exit_status": process.returncode,
        "timed_out": timed_out,
        "lifecycle": lifecycle,
        "rss": {"max_kib_by_role": rss_max, "processes": samples},
        "metrics": metrics,
        "owner_boundary_measurement": routing,
        "handoff_timing": handoff,
        "metrics_error": metrics_error,
        "leftover_processes": leftover,
        "threshold_failures": failures,
        "artifacts": {
            "stdout": str(stdout_path),
            "stderr": str(stderr_path),
            "metrics": str(metrics_path),
        },
        "stdout_marker": stdout_marker,
    }


def run_process_test(
    root: Path, cycle: int, test_name: str, timeout_seconds: int, artifacts: Path
) -> dict[str, Any]:
    """Run one production-worker lifecycle assertion in its own process group.

    The Rust integration tests own the exact route, queue, capability, child, and Unix socket
    cleanup assertions. This wrapper records each bounded invocation without pretending that a
    skipped or timed-out test is lifecycle evidence.
    """

    cycle_dir = artifacts / f"cycle-{cycle:03d}"
    cycle_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = cycle_dir / "stdout.log"
    stderr_path = cycle_dir / "stderr.log"
    command = [
        "cargo",
        "test",
        "--locked",
        "--no-default-features",
        "--features",
        "network-test",
        "--test",
        "routed-process",
        test_name,
        "--",
        "--exact",
        "--test-threads=1",
    ]
    started = time.monotonic()
    timed_out = False
    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
        "w", encoding="utf-8"
    ) as stderr:
        process = subprocess.Popen(
            command,
            cwd=root,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        try:
            process.wait(timeout=timeout_seconds + BUILD_AND_CLEANUP_MARGIN_SECONDS)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
    elapsed_ms = round((time.monotonic() - started) * 1_000)
    failures: list[str] = []
    if process.returncode != 0:
        failures.append(f"production lifecycle test exited with status {process.returncode}")
    if timed_out:
        failures.append("production lifecycle test exceeded its bounded watchdog")
    return {
        "cycle": cycle,
        "status": "pass" if not failures else "fail",
        "elapsed_ms": elapsed_ms,
        "exit_status": process.returncode,
        "timed_out": timed_out,
        "test": test_name,
        "cleanup_assertions": {
            "status": "asserted_by_rust_test_on_pass",
            "children": 0,
            "routes": 0,
            "live_capabilities": 0,
            "packet_queue_frames": 0,
            "packet_queue_bytes": 0,
            "control_queue_frames": 0,
            "control_queue_bytes": 0,
            "runtime_socket_files": 0,
        },
        "threshold_failures": failures,
        "artifacts": {"stdout": str(stdout_path), "stderr": str(stderr_path)},
    }


def run_crash_restart_evidence(
    root: Path, cycles: int, timeout_seconds: int, output: Path, artifacts: Path
) -> dict[str, Any]:
    """Run bounded production crash-isolation and lobby-restart tests.

    Each test starts fresh production Bevy workers; the Rust assertions are the source of truth
    for terminal zero-child/zero-route/zero-queue/socket cleanup. No cycle is reported as passed
    unless the subprocess exits successfully.
    """

    tests = (
        "real_bevy_workers_isolate_match_crash_and_cleanup_routes_peers",
        "real_bevy_lobby_restarts_after_crash_and_cleans_exactly",
    )
    results: list[dict[str, Any]] = []
    for cycle in range(1, cycles + 1):
        for test_name in tests:
            result = run_process_test(root, cycle, test_name, timeout_seconds, artifacts / test_name)
            result["mode"] = "crash-restart"
            results.append(result)
            print(
                f"routed evidence mode=crash-restart cycle={cycle} test={test_name} "
                f"status={result['status']} elapsed_ms={result['elapsed_ms']}",
                file=sys.stderr,
            )
    failures = [
        f"cycle {result['cycle']} {result['test']}: {failure}"
        for result in results
        for failure in result["threshold_failures"]
    ]
    return {
        "schema": SCHEMA,
        "status": "pass" if not failures else "fail",
        "mode": "crash-restart",
        "cycles_requested": cycles,
        "cycles_completed": sum(1 for result in results if result["status"] == "pass") // len(tests),
        "supported_gates": {
            "production_crash_isolation": {
                "status": "measured",
                "cycles": cycles,
                "test": tests[0],
            },
            "production_lobby_restart": {
                "status": "measured",
                "cycles": cycles,
                "test": tests[1],
            },
            "cleanup_and_counters": {
                "status": "asserted_by_each_production_test",
                "children": 0,
                "routes": 0,
                "queued_bytes": 0,
                "socket_files": 0,
            },
        },
        "unsupported_gates": UNSUPPORTED_GATES,
        "threshold_failures": failures,
        "cycles": results,
    }


def main() -> int:
    args = parse_args()
    if not 1 <= args.cycles <= 25:
        raise SystemExit("--cycles must be between 1 and 25")
    if not 1 <= args.timeout <= 120:
        raise SystemExit("--timeout must be between 1 and 120 seconds")
    root = Path(__file__).resolve().parent.parent
    if args.output is None:
        stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        output = root / "target" / f"routed-evidence-{stamp}.json"
    else:
        output = args.output if args.output.is_absolute() else root / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    artifacts = output.parent / f"{output.stem}-artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    if args.mode == "crash-restart":
        summary = run_crash_restart_evidence(root, args.cycles, args.timeout, output, artifacts)
        output.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(json.dumps(summary, indent=2, sort_keys=True))
        print(f"routed evidence summary: {output}", file=sys.stderr)
        return 0 if summary["status"] == "pass" else 1

    if args.mode == "both":
        modes = ("wipeout", "hot-zone")
    elif args.mode == "all":
        modes = ("wipeout", "hot-zone", "heist")
    else:
        modes = (args.mode,)
    cycles: list[dict[str, Any]] = []
    for mode in modes:
        for cycle in range(1, args.cycles + 1):
            result = run_cycle(root, cycle, mode, args.timeout, artifacts / mode)
            result["cycle_key"] = f"{mode}-{cycle:03d}"
            cycles.append(result)
            print(
                f"routed evidence mode={mode} cycle={cycle} status={result['status']} "
                f"elapsed_ms={result['elapsed_ms']} ready_ms={result['lifecycle']['ready_ms']}",
                file=sys.stderr,
            )
    ready_values = [
        ready_ms
        for cycle in cycles
        for ready_ms in cycle["lifecycle"]["ready_ms"]
    ]
    handoff_samples = [
        sample
        for cycle in cycles
        for sample in cycle["handoff_timing"]["samples"]
    ]
    handoff_values = [sample["handoff_ms"] for sample in handoff_samples]
    forced_samples = [
        cycle["lifecycle"]["forced_total_duration_ms"]
        for cycle in cycles
        if cycle["lifecycle"]["forced_total_duration_status"] == "measured"
    ]
    max_rss = {
        role: max(
            (
                cycle["rss"]["max_kib_by_role"][role]
                for cycle in cycles
                if cycle["rss"]["max_kib_by_role"][role] is not None
            ),
            default=None,
        )
        for role in RSS_LIMITS_KIB
    }
    failures = [
        f"{cycle['mode']} cycle {cycle['cycle']}: {failure}"
        for cycle in cycles
        for failure in cycle["threshold_failures"]
    ]
    if ready_values and p95(ready_values) is not None and p95(ready_values) > READY_P95_TARGET_MS:
        failures.append(
            f"aggregate spawn-to-ready p95 exceeded {READY_P95_TARGET_MS}ms: "
            f"{p95(ready_values)}ms"
        )
    if (
        len(handoff_values) >= 20
        and p95(handoff_values) is not None
        and p95(handoff_values) > HANDOFF_P95_TARGET_MS
    ):
        failures.append(
            f"aggregate allocation-to-connected p95 exceeded {HANDOFF_P95_TARGET_MS}ms: "
            f"{p95(handoff_values)}ms"
        )
    owner_boundary_measurements = [
        cycle["owner_boundary_measurement"]
        for cycle in cycles
        if cycle["owner_boundary_measurement"] is not None
    ]
    owner_boundary_public_samples = sum(
        measurement["public_receive_to_packet_ipc_enqueue"]["sample_count"]
        for measurement in owner_boundary_measurements
    )
    owner_boundary_worker_samples = sum(
        measurement["worker_packet_to_public_send"]["sample_count"]
        for measurement in owner_boundary_measurements
    )
    owner_boundary_max_cycle_p95 = {
        "public_receive_to_packet_ipc_enqueue": max(
            (
                measurement["public_receive_to_packet_ipc_enqueue"]["p95_us"]
                for measurement in owner_boundary_measurements
                if measurement["public_receive_to_packet_ipc_enqueue"]["p95_us"] is not None
            ),
            default=None,
        ),
        "worker_packet_to_public_send": max(
            (
                measurement["worker_packet_to_public_send"]["p95_us"]
                for measurement in owner_boundary_measurements
                if measurement["worker_packet_to_public_send"]["p95_us"] is not None
            ),
            default=None,
        ),
    }
    owner_boundary_status = "measured_diagnostic" if owner_boundary_measurements else "unsupported"
    traffic_validations = [
        measurement["public_traffic_validation"]
        for measurement in owner_boundary_measurements
    ]
    traffic_status = (
        "measured_public_exact_ipc_unsupported"
        if traffic_validations
        else "unsupported"
    )
    summary = {
        "schema": SCHEMA,
        "status": "pass" if not failures else "fail",
        "mode": args.mode,
        "modes_requested": list(modes),
        "cycles_per_mode": args.cycles,
        "cycles_requested": len(modes) * args.cycles,
        "cycles_completed": len(cycles),
        "supported_gates": {
            "spawn_to_ready": {
                "status": "measured",
                "hard_max_ms": READY_HARD_LIMIT_MS,
                "target_p95_ms": READY_P95_TARGET_MS,
                "sample_count": len(ready_values),
                "p95_ms": p95(ready_values),
                "max_ms": max(ready_values, default=None),
            },
            "graceful_stop_reap_presence": {
                "status": "measured_presence",
                "persistent_lobby_ready_exactly_once_per_cycle": 1,
                "dynamic_match_workers_exactly_one_per_cycle": 1,
                "result_received_exactly_once_per_match": 1,
                "result_precedes_successful_reap_and_graceful_stop": True,
                "observed_terminal_log_order": [
                    "spawned",
                    "ready",
                    "result-received",
                    "reaped",
                    "stopped(forced=false)",
                    "cleaned",
                ],
                "duration_status": (
                    "measured"
                    if all(
                        cycle["lifecycle"]["graceful_stop_reap_duration_status"] == "measured"
                        for cycle in cycles
                    )
                    else "invalid"
                ),
                "graceful_stop_reap_limit_ms": GRACEFUL_STOP_LIMIT_MS,
                "graceful_stop_reap_ms": [
                    cycle["lifecycle"]["graceful_stop_reap_duration_ms"]
                    for cycle in cycles
                ],
            },
            "persistent_lobby_graceful_shutdown": {
                "status": (
                    "measured_presence"
                    if all(
                        cycle["lifecycle"]["lobby_shutdown_status"] == "pass"
                        for cycle in cycles
                    )
                    else "invalid"
                ),
                "required_terminal_log_order": [
                    "ready",
                    "stop-requested",
                    "stop-sent",
                    "reaped(success=true)",
                    "stopped(forced=false)",
                    "cleaned",
                ],
                "successful_reap_required": True,
                "forced_stop_allowed": False,
                "cleaned_exactly_once_required": True,
                "cycle_statuses": [
                    cycle["lifecycle"]["lobby_shutdown_status"] for cycle in cycles
                ],
            },
            "allocation_to_connected": {
                "status": "measured" if handoff_samples else "invalid",
                "hard_max_ms": HANDOFF_HARD_LIMIT_MS,
                "target_p95_ms": HANDOFF_P95_TARGET_MS,
                "target_p95_min_samples": 20,
                "sample_count": len(handoff_values),
                "p95_ms": p95(handoff_values),
                "max_ms": max(handoff_values, default=None),
                "samples": handoff_samples,
            },
            "forced_stop_total": {
                "status": "measured" if forced_samples else "not_exercised",
                "hard_max_ms": FORCED_STOP_TOTAL_LIMIT_MS,
                "sample_count": len(forced_samples),
                "max_ms": max(forced_samples, default=None),
            },
            "rss": {"status": "measured", "limits_kib": RSS_LIMITS_KIB, "max_kib": max_rss},
            "cleanup_and_counters": {"status": "measured"},
            "owner_boundary_processing_queue_latency": {
                "status": owner_boundary_status,
                "threshold_status": "diagnostic_only",
                "sample_count_by_direction": {
                    "public_receive_to_packet_ipc_enqueue": owner_boundary_public_samples,
                    "worker_packet_to_public_send": owner_boundary_worker_samples,
                },
                "max_cycle_p95_us_by_direction": owner_boundary_max_cycle_p95,
                "scope": (
                    "supervisor owner-boundary processing/queue intervals; not end-to-end "
                    "and not paired to direct UDP"
                ),
            },
            "directional_traffic_accounting": {
                "status": traffic_status,
                "public_envelope_overhead_bytes": PUBLIC_ENVELOPE_OVERHEAD_BYTES,
                "public_formula": "public_bytes = inner_bytes + 42 * datagrams",
                "public_formula_status": (
                    "pass" if traffic_validations and all(
                        validation["status"] == "pass" for validation in traffic_validations
                    ) else "unsupported"
                ),
                "ipc_exact_overhead_status": "unsupported",
                "scope": "public/inner boundary counters; IPC counters remain mixed packet/control",
            },
        },
        "unsupported_gates": UNSUPPORTED_GATES,
        "aggregate": {
            "ready_ms": ready_values,
            "max_rss_kib_by_role": max_rss,
            "forced_stops": sum(cycle["lifecycle"]["forced_stop"] for cycle in cycles),
            "owner_boundary_max_cycle_p95_us_by_direction": owner_boundary_max_cycle_p95,
            "leftover_processes": [
                pid for cycle in cycles for pid in cycle["leftover_processes"]
            ],
        },
        "threshold_failures": failures,
        "cycles": cycles,
    }
    output.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if not args.keep_artifacts:
        # Logs remain useful for a failed run, so only report their paths. The caller can remove
        # the artifact directory explicitly after inspection; the evidence JSON stays durable.
        pass
    print(json.dumps(summary, indent=2, sort_keys=True))
    print(f"routed evidence summary: {output}", file=sys.stderr)
    return 0 if summary["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
