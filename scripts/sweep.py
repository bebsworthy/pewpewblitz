#!/usr/bin/env python3
"""Prune stale Cargo artifacts and agent scratch directories from `target/` (BRL-r8ev).

Usage: sweep.py [dry | --dry-run] [--days N | N]

Removes, when the newest file inside is older than N days (default 2):
  - `target/debug/incremental/<crate>-<hash>` directories that are not the newest one for
    their crate, and stale non-newest sessions inside the directories that stay;
  - `target/debug/deps` artifact groups (every file sharing one `<name>-<hash>`) that are not
    the newest group for their crate;
  - top-level `target/brl*` evidence and `target/agent-*` scratch target directories.

Never touches `target/dev`, `target/e2e`, `target/release`, the Android outputs, files under
`target/debug/` itself (the binaries `just server` runs), any artifact hard-linked to one of
them, or the executable of any running process. `data/` is outside `target/` and never read.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

DEFAULT_DAYS = 2
DEPS_NAME = re.compile(r"^(?P<name>.+?)-(?P<hash>[0-9a-f]{16})(?:\..*)?$")
INCREMENTAL_NAME = re.compile(r"^(?P<name>.+)-(?P<hash>[0-9a-z]{13})$")
SESSION_NAME = re.compile(r"^(?P<session>s-[0-9a-z]+-[0-9a-z]+)(?:-.+|\.lock)$")
SCRATCH_NAME = re.compile(r"^(brl|agent-)")


def usage(message: str) -> None:
    print(f"just sweep: {message}", file=sys.stderr)
    print("usage: just sweep [dry | --dry-run] [--days N | N]", file=sys.stderr)
    sys.exit(2)


def parse_args(argv: list[str]) -> tuple[bool, int]:
    dry_run, days = False, DEFAULT_DAYS
    args = iter(argv)
    for arg in args:
        if arg in ("dry", "--dry-run", "-n"):
            dry_run = True
        elif arg == "--days":
            value = next(args, None)
            if value is None or not value.isdigit():
                usage("--days needs a whole number of days")
            days = int(value)
        elif arg.isdigit():
            days = int(arg)
        else:
            usage(f"unknown argument {arg!r}")
    return dry_run, days


class Entry:
    """One removable path: a file, or a directory with everything beneath it."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.newest = 0.0
        self.inodes: dict[tuple[int, int], tuple[int, int]] = {}
        self._scan(path)

    def _record(self, path: Path) -> None:
        try:
            stat = path.lstat()
        except FileNotFoundError:
            return
        self.newest = max(self.newest, stat.st_mtime)
        self.inodes[(stat.st_dev, stat.st_ino)] = (stat.st_nlink, stat.st_blocks * 512)

    def _scan(self, path: Path) -> None:
        self._record(path)
        if path.is_dir() and not path.is_symlink():
            for root, dirs, files in os.walk(path):
                for name in dirs + files:
                    self._record(Path(root, name))


def running_inodes(target: Path) -> set[tuple[int, int]]:
    """Inodes of every executable a running process has mapped from inside `target/`."""
    try:
        output = subprocess.run(
            ["lsof", "-n", "-P", "-w", "-d", "txt", "-Fn"],
            capture_output=True,
            text=True,
            check=False,
        ).stdout
    except FileNotFoundError:
        usage("lsof is required to protect running binaries")
    protected = set()
    root = str(target.resolve())
    for line in output.splitlines():
        if line.startswith("n") and line[1:].startswith(root):
            try:
                stat = os.stat(line[1:])
            except OSError:
                continue
            protected.add((stat.st_dev, stat.st_ino))
    return protected


def uplifted_inodes(profile: Path) -> set[tuple[int, int]]:
    """Inodes of the top-level outputs (`target/debug/brawler-server`, ...), hard-linked from deps."""
    protected = set()
    if profile.is_dir():
        for child in profile.iterdir():
            if child.is_file():
                stat = child.stat()
                protected.add((stat.st_dev, stat.st_ino))
    return protected


def stale_groups(
    groups: dict[str, dict[str, list[Path]]], cutoff: float
) -> list[tuple[str, list[Entry]]]:
    """For each crate, every artifact group older than `cutoff` except the newest group."""
    stale = []
    for crate, by_hash in groups.items():
        scanned = {key: [Entry(path) for path in paths] for key, paths in by_hash.items()}
        newest = {key: max(entry.newest for entry in entries) for key, entries in scanned.items()}
        keep = max(newest, key=newest.__getitem__)
        for key, entries in scanned.items():
            if key != keep and newest[key] < cutoff:
                stale.append((crate, entries))
    return stale


def deps_candidates(deps: Path, cutoff: float) -> list[list[Entry]]:
    by_hash: dict[str, list[Path]] = defaultdict(list)
    crate_of: dict[str, str] = {}
    for child in deps.iterdir():
        match = DEPS_NAME.match(child.name)
        if not match:
            continue
        key, name = match["hash"], match["name"]
        by_hash[key].append(child)
        # `.d` files never carry the `lib` prefix, so they name the crate unambiguously.
        if child.suffix == ".d":
            crate_of[key] = name
        elif key not in crate_of:
            crate_of[key] = name.removeprefix("lib")
    groups: dict[str, dict[str, list[Path]]] = defaultdict(dict)
    for key, paths in by_hash.items():
        groups[crate_of[key]][key] = paths
    return [entries for _, entries in stale_groups(groups, cutoff)]


def incremental_candidates(incremental: Path, cutoff: float) -> list[list[Entry]]:
    groups: dict[str, dict[str, list[Path]]] = defaultdict(dict)
    for child in incremental.iterdir():
        match = INCREMENTAL_NAME.match(child.name)
        if match and child.is_dir():
            groups[match["name"]][match["hash"]] = [child]
    candidates = [entries for _, entries in stale_groups(groups, cutoff)]
    removed = {entry.path for entries in candidates for entry in entries}
    # Inside the directories that stay, drop old sessions other than the newest one.
    for by_hash in groups.values():
        for (directory,) in by_hash.values():
            if directory in removed:
                continue
            sessions: dict[str, list[Path]] = defaultdict(list)
            for child in directory.iterdir():
                match = SESSION_NAME.match(child.name)
                if match:
                    sessions[match["session"]].append(child)
            if len(sessions) > 1:
                stale = stale_groups({directory.name: sessions}, cutoff)
                candidates += [entries for _, entries in stale]
    return candidates


def scratch_candidates(target: Path, cutoff: float) -> list[list[Entry]]:
    entries = [Entry(child) for child in sorted(target.iterdir()) if SCRATCH_NAME.match(child.name)]
    return [[entry] for entry in entries if entry.newest < cutoff]


def freed_bytes(entries: list[Entry]) -> int:
    """Bytes actually returned to the disk: inodes whose every hard link is being removed."""
    seen: dict[tuple[int, int], int] = defaultdict(int)
    info: dict[tuple[int, int], tuple[int, int]] = {}
    for entry in entries:
        for inode, value in entry.inodes.items():
            seen[inode] += 1
            info[inode] = value
    return sum(size for inode, (links, size) in info.items() if seen[inode] >= links)


def human(size: int) -> str:
    if size < 1 << 30:
        return f"{size / (1 << 20):.1f} MB"
    return f"{size / (1 << 30):.1f} GB"


def remove(entry: Entry) -> None:
    if entry.path.is_dir() and not entry.path.is_symlink():
        shutil.rmtree(entry.path, ignore_errors=True)
    else:
        entry.path.unlink(missing_ok=True)


def main() -> None:
    dry_run, days = parse_args(sys.argv[1:])
    project = Path(__file__).resolve().parent.parent
    target = project / "target"
    if not target.is_dir():
        print("just sweep: no target/ directory; nothing to do")
        return
    cutoff = time.time() - days * 86_400
    debug = target / "debug"
    protected = running_inodes(target) | uplifted_inodes(debug)

    categories: list[tuple[str, list[list[Entry]]]] = []
    if (debug / "incremental").is_dir():
        categories.append(("debug/incremental", incremental_candidates(debug / "incremental", cutoff)))
    if (debug / "deps").is_dir():
        categories.append(("debug/deps", deps_candidates(debug / "deps", cutoff)))
    categories.append(("evidence and scratch (brl*, agent-*)", scratch_candidates(target, cutoff)))

    verb = "would remove" if dry_run else "removed"
    # rustc hard-links incremental work products into deps/, so space freed by removing both
    # only shows up in the combined total; the per-category figures are lower bounds.
    everything: list[Entry] = []
    for label, groups in categories:
        # A group (one artifact hash or session) stays whole if any file in it is protected.
        removable = [
            group for group in groups if all(protected.isdisjoint(e.inodes) for e in group)
        ]
        skipped = len(groups) - len(removable)
        selected = [entry for group in removable for entry in group]
        size = freed_bytes(selected)
        everything += selected
        note = f" (kept {skipped} holding a protected or running binary)" if skipped else ""
        print(f"{label}: {verb} {len(selected)} entries, {human(size)}{note}")
        for entry in selected:
            if label.startswith("evidence") or (dry_run and len(selected) <= 40):
                print(f"  {entry.path.relative_to(project)}")
            if not dry_run:
                remove(entry)
    total = human(freed_bytes(everything))
    print(f"total: {verb} {total} (artifacts older than {days} days; newest per crate kept)")
    if dry_run:
        print("dry run: nothing deleted; run `just sweep` to remove")


if __name__ == "__main__":
    main()
