#!/usr/bin/env python3
"""Measure supervisor/lobby/match RSS and CPU under concurrent headless Practice matches.

Usage: capacity.py <repo> <out-dir> <game-type>...
Starts the release supervisor on 127.0.0.1:5000, samples every 0.5 s by process role,
launches one headless Practice client per game type concurrently, and stops when all
clients exit (or after 300 s). Prints a per-role summary and writes samples.csv.
"""
import csv, os, signal, subprocess, sys, time

repo, out = sys.argv[1], sys.argv[2]
games = sys.argv[3:]
os.makedirs(out, exist_ok=True)
data = os.path.join(out, "server")
subprocess.run(["rm", "-rf", data], check=True)
rel = os.path.join(repo, "target/release")
ident = dict(l.split("=", 1) for l in subprocess.check_output(
    [f"{rel}/brawler-server", "routing-identity"], text=True).split() if "=" in l)
sup_log = open(os.path.join(out, "supervisor.log"), "w")
sup = subprocess.Popen([f"{rel}/brawler-supervisor",
    "--network-protocol", ident["network_protocol"],
    "--protocol-registry-fingerprint", ident["protocol_registry_fingerprint"],
    "--content-fingerprint", ident["content_fingerprint"],
    "--worker-executable", f"{rel}/brawler-server",
    "--game-types", os.path.join(repo, "config/server/game-types.ron"),
    "--data-directory", data, "--bind", "127.0.0.1:5000"],
    stderr=sup_log, stdout=sup_log, cwd=repo)

def cpu_secs(t):
    parts = t.replace("-", ":").split(":")
    secs = 0.0
    for p in parts:
        secs = secs * 60 + float(p)
    return secs

def snapshot():
    rows = subprocess.check_output(["ps", "-ax", "-o", "pid=,ppid=,rss=,time=,command="], text=True)
    procs = {}
    for line in rows.splitlines():
        f = line.split(None, 4)
        if len(f) < 5: continue
        pid, ppid = int(f[0]), int(f[1])
        if pid == sup.pid:
            role = "supervisor"
        elif ppid == sup.pid and "brawler-server" in f[4]:
            role = "worker"
        else:
            continue
        procs[pid] = (role, int(f[2]), cpu_secs(f[3]))
    return procs

samples, prev, t0 = [], {}, time.monotonic()
def sample(phase):
    global prev
    now = time.monotonic() - t0
    cur = snapshot()
    for pid, (role, rss, cpu) in cur.items():
        last = prev.get(pid)
        pct = None
        if last:
            dt = now - last[0]
            pct = 100.0 * (cpu - last[1]) / dt if dt > 0 else None
        samples.append((round(now, 2), phase, pid, role, rss, pct))
    prev = {pid: (now, cpu) for pid, (_, _, cpu) in cur.items()}

# Lobby idle.
for _ in range(40):
    sample("idle"); time.sleep(0.5)

clients = []
for i, g in enumerate(games):
    log = open(os.path.join(out, f"client-{g}.log"), "w")
    clients.append(subprocess.Popen([os.path.join(repo, "target/debug/brawler-client"),
        "--client-id", str(7000 + i), "--server", "127.0.0.1:5000", "--transport", "routed-udp",
        "--auto-connect", "--headless", "--product-practice-smoke", "--product-game-type", g],
        stdout=log, stderr=log, cwd=repo))
    # The lobby admits one match start at a time; stagger so each start completes.
    for _ in range(int(float(os.environ.get("STAGGER_S", "0")) * 2)):
        sample("load"); time.sleep(0.5)
deadline = time.monotonic() + 300
while time.monotonic() < deadline and any(c.poll() is None for c in clients):
    sample("load"); time.sleep(0.5)
codes = [c.poll() for c in clients]
for c in clients:
    if c.poll() is None: c.kill()
for _ in range(10):
    sample("after"); time.sleep(0.5)
sup.send_signal(signal.SIGINT)
try: sup.wait(timeout=10)
except subprocess.TimeoutExpired: sup.kill()

with open(os.path.join(out, "samples.csv"), "w", newline="") as fh:
    w = csv.writer(fh); w.writerow(["t", "phase", "pid", "role", "rss_kib", "cpu_pct"]); w.writerows(samples)

# The lobby is the worker present during idle; matches are workers first seen under load.
idle_workers = {s[2] for s in samples if s[1] == "idle" and s[3] == "worker"}
def label(s):
    if s[3] == "supervisor": return "supervisor"
    return "lobby" if s[2] in idle_workers else "match"
print("client exit codes:", dict(zip(games, codes)), "duration_s:", round(time.monotonic() - t0, 1))
for phase in ("idle", "load"):
    for role in ("supervisor", "lobby", "match"):
        rs = [s for s in samples if s[1] == phase and label(s) == role]
        if not rs: continue
        rss = [s[4] for s in rs]; cpu = sorted(s[5] for s in rs if s[5] is not None)
        n = len({s[2] for s in rs})
        p95 = cpu[int(len(cpu) * 0.95)] if cpu else 0
        mean = sum(cpu) / len(cpu) if cpu else 0
        print(f"{phase:5} {role:10} procs={n} rss_max={max(rss)/1024:.1f}MiB cpu_mean={mean:.1f}% cpu_p95={p95:.1f}%")
# Whole-host totals per sample time under load.
by_t = {}
for s in samples:
    if s[1] != "load": continue
    by_t.setdefault(s[0], [0, 0.0]); by_t[s[0]][0] += s[4]; by_t[s[0]][1] += s[5] or 0
if by_t:
    print(f"load  total      rss_max={max(v[0] for v in by_t.values())/1024:.1f}MiB cpu_max={max(v[1] for v in by_t.values()):.1f}% cpu_mean={sum(v[1] for v in by_t.values())/len(by_t):.1f}%")
