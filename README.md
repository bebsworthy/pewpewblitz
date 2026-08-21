# Brawler

Brawler is a server-authoritative top-down arena shooter built around player-authored fighter builds.
V1 completed on 2026-08-18 as a gameplay MVP: Wipeout and Hot Zone, typed weapon/build/map recipes,
four weapon profiles, bounded passives and ultimates, generated arenas, quantized destructible
terrain, replicated match/map/combat state, local input settings, and client-only presentation over
Lightyear Netcode/UDP.

V1 completion is not a release-ready claim. Controller feel, audio, HUD/readability, balance, match
pacing, and related tuning remain explicit pre-release polish. V2 completed on 2026-08-20 with the
product client flow, single-public-port routed supervisor, isolated lobby and match workers,
server-local matchmaking, concurrent match lifecycle, product HUD, and server-hosted practice. See
the [completed v2 roadmap](docs/implementation/v2/roadmap.md),
[v2 closeout milestone](docs/implementation/v2/milestone-09.md), and
[completed v1 roadmap](docs/implementation/v1/roadmap.md).

V3 completed on 2026-08-20. The supported client now presents the gameplay world as a fixed-camera
3D scene while preserving planar authority, Avian 2D collision, the existing protocol,
the routed server topology, and the Bevy UI shell. Imported Kenney GLBs provide the first fighter,
weapon, and cover families; cached primitives and generated meshes retain exact dynamic geometry
and deterministic fallbacks. See the [completed V3 roadmap](docs/implementation/v3/roadmap.md) and
[V3 closeout milestone](docs/implementation/v3/milestone-04.md).

V4 M01 is in feedback review with the accepted fixed-perspective correction. Its scope turns the V3
renderer into a reusable map-presentation and content foundation: a game-object taxonomy mapped to curated visual assets,
themed ground and modular decorated borders, one document per map, and a second map/theme proof.
The player-facing map editor is deferred to the root backlog. The
[V4 roadmap](docs/implementation/v4/roadmap.md) and
[M01 specification](docs/implementation/v4/milestone-01.md) define the active feedback contract.

## Toolchain

The repository pins Rust 1.95.0 in [`rust-toolchain.toml`](rust-toolchain.toml). Bevy is pinned to 0.19.1 and Lightyear to 0.29.0. `Cargo.lock` is committed and must be updated intentionally.

## Canonical commands

The everyday development surface is intentionally small:

```sh
just
just server
just client
just run <client-count>

just fmt
just check
just lint
just test
just e2e [client-count]
just v3-render-evidence [report-path]
just ci
just clean
```

`just server` starts the routed supervisor and production lobby at `127.0.0.1:5000`. `just client`
opens one normal product client against that address. `just run <client-count>` builds once, starts
the routed server, and opens exactly that many interactive clients; counts from 1 through 16 are
accepted so partial queues and disconnects are easy to reproduce. Press Ctrl-C to stop the complete
local process tree.

`just v3-render-evidence` builds release client/server/supervisor binaries, runs two routed native
clients at 1280×720, records a bounded 10-second warm-up plus 30-second measurement, and writes
`target/v3-render-evidence.txt` without overwriting an existing report. Set
`BRAWLER_RENDER_MODE=hot-zone` for the second mode or pass a different report path.

`just test` owns all deterministic Rust suites, including routing, client, server, network, and
performance tests. `just e2e` runs the shortest real-process product path with two clients and First
Blood; pass `4` or `6` to exercise the 2v2 or 3v3 path. `just ci` runs formatting, Clippy, server
feature isolation, all deterministic tests, and the complete 2/4/6-client E2E matrix. Focused Cargo
commands and scripts remain available for diagnostics, but they are deliberately not separate
top-level `just` recipes. E2E runs choose an unused loopback port by default, so they can run beside
an interactive server; set `BRAWLER_ROUTED_BIND` only when a fixed test address is required.

Normal client startup opens the product Title without connecting. Play opens Server Select for
multiplayer; Practice uses the same server connection and advertised game types but immediately
starts a server-hosted authoritative match with inert `Bot N` fighters filling the roster. Neither
path launches server processes from the client. The first-run address is `127.0.0.1:5000`.

Server game types are authored in `config/server/game-types.ron`. Each entry owns flat match rules:
Wipeout uses `kills_to_win`, Hot Zone uses `capture_seconds`, and every entry declares
`match_duration_seconds`, `countdown_seconds`, and `respawn_seconds`. There is no shared defaults
block or operator-facing rules profile; startup validates and passes the resolved values to the
authoritative match worker.

`python3 scripts/network-routed-evidence.py` runs bounded cold routed-process cycles.
It records the exact
Result-driven worker lifecycle, per-role RSS, directional public/inner/IPC traffic, bounded routing
owner-loop latency diagnostics, final route/queue/drop counters, and process cleanup in
`target/routed-evidence-<UTC timestamp>.json`. The report explicitly marks paired CPU/direct
bandwidth, full IPC-to-worker latency, packet-only IPC overhead, packet-capture MTU, and paired
fixed-tick gates unsupported; correlated stop/reap and allocation-to-connected samples remain
below their required campaign cardinalities. It never fabricates those measurements. Add
`--keep-artifacts` by invoking `python3 scripts/network-routed-evidence.py --keep-artifacts` to
retain per-cycle logs.

`BRAWLER_NETWORK_HEADLESS=1 BRAWLER_ROUTED_BIND="[::1]:5000" ./scripts/network-routed.sh` runs the
same production routed process check over `[::1]`; the
client derives an IPv6 local socket from the selected `--server` address, or accepts an explicit
`--local-addr`. On macOS, `./scripts/network-routed-capture.sh target/routed-capture.pcap` runs
both IPv4 and IPv6 headless smokes under `tcpdump` on `lo0` and parses the resulting classic pcap with
`scripts/verify-routed-capture.py`. BPF capture permission may require an approved administrator
session. No capture result is considered evidence unless a real pcap is produced and the parser
observes IPv4/IPv6 UDP payloads no larger than 1,200 bytes with no IPv4 fragmentation or IPv6
Fragment header; unavailable or malformed captures stay unsupported.

`python3 scripts/network-paired-evidence.py --pairs 1 --timeout 90 --mode wipeout` runs the bounded
one-pair M01 comparison smoke. The historical M01 gate used three pairs. It runs the existing
direct and routed verification launchers sequentially on the same host, source tree, mode, and
verification rules, requires the exact expected process-role cardinalities, samples every Brawler process's CPU time and RSS, and writes
`target/paired-evidence-<UTC timestamp>.json`. Aggregate routed CPU must be no more than 20% over
the direct aggregate when both process series and a correlated common observation interval are
comparable. Direct server transport bytes are compared with routed supervisor match-worker inner
ingress/egress bytes independently, with a 10% limit per direction and for the total. Routed public-envelope and mixed packet/control IPC bytes are
reported as overhead diagnostics and are never compared with direct gameplay bytes. Missing
comparable samples or common-window checkpoints produce an explicit `unsupported` result. Run
`python3 -m unittest scripts/test_network_paired_evidence.py` for the parser and threshold-gate
tests without starting processes.

The server accepts `--bind`, `--max-clients`, and `--handshake-timeout-ms`. A normal windowed client
starts at the controller-friendly Title screen and does not connect; `--client-id` defaults to 1 in
that offline shell. `--auto-connect` selects the established development/network path and requires
an explicit `--client-id`. The client also accepts `--server`, `--local-addr`, and `--build-preset 1..5` (`1` Runner, `2` Bruiser, `3` Controller,
`4` Duelist, `5` the default legal custom Pulse), plus bounded automation flags `--headless --exit-after-roster 2
--move-axis X,Y --aim-axis X,Y --aim-dummy --fire --ultimate --simulation-ticks N`. `--combat-demo` enables the
same authoritative aim-at-dummy/fire loop in a windowed client for a reproducible visual smoke run.
Use `--window-size WIDTHxHEIGHT` to reproduce a supported visual-check layout.
For the legacy direct-UDP visual harness, `BRAWLER_NETWORK_SCREENSHOT_DIR=<DIR>` captures the first
windowed client's scheduled frame through the same built-in screenshot path; set
`BRAWLER_NETWORK_SCREENSHOT_FIRST=<update>` to capture after countdown/startup.
Set `BRAWLER_FORCE_PRIMITIVE_WORLD=1` on a windowed client to verify the deterministic primitive
fallbacks without modifying the packaged optional Kenney assets.
On macOS, `scripts/macos-client-bundle.sh` creates a temporary addressable `.app` wrapper around the
already-built client for native visual automation; it prints the wrapper path and does not modify the
production application composition.
`--controller-demo` creates a synthetic gamepad only for the windowed controller-path smoke; it
still uses the normal gamepad sampler and native input buffer, but does not substitute for a
physical controller.
`RUST_LOG` controls log filtering, for example `RUST_LOG=brawler=info`. Window titles identify the two clients; structured logs report
connection outcome and stable `(player_id, network_entity_id)` roster entries. `just e2e` is the
canonical routed product check. The retained `scripts/network.sh` is the legacy direct-UDP baseline
until its roadmap retirement gate.

For the supervised combat path, use `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ASSERT_COMBAT=1
./scripts/network.sh`. Its legacy combat verifier composes an explicit test-only dummy fixture and
waits for server-verified shots, hits, damage, defeat, reset, and both client observations; production
Wipeout composition has no practice dummy. Use `scripts/network-match.sh` for the current four-client
match, respawn, telemetry, and restart process gate. It defaults to the shortened verification rules;
set `BRAWLER_NETWORK_WIPEOUT_RULES=production`, raise `BRAWLER_NETWORK_SIMULATION_TICKS`, and set a
matching `BRAWLER_NETWORK_MATCH_TIMEOUT_SECONDS` for a controlled normal-duration comparison.
`BRAWLER_NETWORK_PROFILE=local|typical|adverse` applies the corresponding Lightyear receive
conditioner; `./scripts/network-combat-profiles.sh` repeats all three profiles and reports median/p95
convergence timings.

The network launcher waits for its own server's readiness signal before starting clients, so a bind
collision cannot accidentally test a pre-existing server. It supervises the already-built binaries
directly, so Ctrl-C terminates the actual server/client processes before the launcher exits. It
remains running when one windowed client closes so the remaining roster can be observed. Restart
that client with the same individual command and `--client-id`; set
`BRAWLER_NETWORK_TIMEOUT_SECONDS` to add a bounded windowed-session deadline when needed.

The completed v1 milestones provide movement, combat, authored arenas, Wipeout and Hot Zone,
bounded brawler builds/abilities, destructible terrain, and closeout diagnostics. At build selection,
use Left/Right or A/D to choose a preset and Space/Enter
to confirm; on a controller use the D-pad or left stick and South. On Custom, Up/Down selects one of
six fields and Left/Right changes its value; Escape or East returns to Runner. In Waiting,
Space/Enter or South readies the participant, and the same input
requests the next match after the completed-phase lock. During play, use WASD to move, mouse position
to aim, mouse-left to fire, and E to use the charged ultimate; Q remains reserved for the future
active-item slot. A connected controller uses the left stick for movement, right stick for aim,
right trigger to fire, right bumper for the ultimate, and Start for pause. Hold Tab or controller
Select for the full roster scoreboard. Pausing (Escape or Start) also opens the local settings
overlay: Tab or the D-pad cycles calibration and binding rows, brackets or the D-pad adjust values
(move/aim deadzones, aim commit, trigger thresholds), B or South rebinds the selected row from the
next key, mouse-button, or controller-button press, I/O toggle Y-axis inversion, and R restores
the validated defaults; session-local settings shape device input before quantization and never
reach the server. The HUD shows match phase, score/time/result,
roster/loadout/readiness, respawn and protection state, health, ammo, ultimate meter/phase, passive
state, sentry health/lifetime, and cooldown/reload. Camera-projected fighter UI shows relation-colored
names, rounded relation-aware health, a white health value, and local-only segmented ammunition.
The arena is reconstructed from the authoritative replicated map snapshot. Its perimeter and cover
block fighters and weapon delivery, while 3D visuals, audio, and HUD state remain presentation-only.
The legacy direct-UDP diagnostic script still accepts `BRAWLER_NETWORK_WINDOWED_COMBAT_DEMO=1`,
weapon presets, controller-demo selection, and `BRAWLER_RENDER_PROFILE=30|60|high` when a focused
v1 comparison is needed. These are diagnostic parameters rather than everyday development recipes;
the fixed authoritative simulation remains 60 Hz.

For a focused live movement trace, run
`BRAWLER_INPUT_TRACE=1 RUST_LOG=brawler=info just run 1`. The trace reports focused-window WASD
sampling, the Lightyear input target, authoritative server movement, replicated interpolation
history, and the final client presentation pose only when those states materially change.

Do not use `--all-features` as a supported application build: client and server are independently tested production configurations, while `network-test` is the dedicated separate-app Crossbeam integration configuration. Cargo features are additive.

## Repository conventions

Authoritative authored gameplay data lives under `content/v1/` and is compiled into both roles.
Client-only runtime art/audio lives under `assets/brawler/`; exact source and CC0 provenance are
recorded in `assets/manifest.ron` with retained source license texts under `assets/licenses/`.
The active implementation scope is always the next validated milestone file. V4 M01 is currently
in user playtest; its roadmap and milestone define the presentation/asset feedback work, while
deferred release polish remains visible in the completed V3 roadmap and root backlog.
