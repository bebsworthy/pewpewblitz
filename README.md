# Brawler

Brawler is a server-authoritative top-down arena shooter built around player-authored fighter builds.
V1 completed on 2026-08-18 as a gameplay MVP: Wipeout and Hot Zone, typed weapon/build/map recipes,
four weapon profiles, bounded passives and ultimates, generated arenas, quantized destructible
terrain, replicated match/map/combat state, local input settings, and client-only presentation over
Lightyear Netcode/UDP.

V1 completion is not a release-ready claim. Controller feel, audio, HUD/readability, balance, match
pacing, and related tuning remain explicit pre-release polish. V2 M01 is implementing a
single-public-port routed supervisor with isolated lobby and match workers after specification
validation on 2026-08-18. See the [v2 roadmap](docs/implementation/v2/roadmap.md),
[active milestone](docs/implementation/v2/milestone-01.md), and [completed v1 roadmap](docs/implementation/v1/roadmap.md).

## Toolchain

The repository pins Rust 1.95.0 in [`rust-toolchain.toml`](rust-toolchain.toml). Bevy is pinned to 0.19.1 and Lightyear to 0.29.0. `Cargo.lock` is committed and must be updated intentionally.

## Canonical commands

Run these from the repository root:

```sh
just
```

`just` lists the development recipes. Use `just run` for one server and one client, `just network`
for one server and two distinguishable client windows, `just network-combat` for two windows with
client 1 automatically firing at the test-only neutral dummy, `just network-controller` for a synthetic controller-path
window smoke, or `just network-smoke` for a bounded headless process check.
`just verify` runs the automated development-cycle gates, and `just user-test`
runs verification first and then starts the interactive end-of-cycle scenario. Close either window
or press Ctrl-C when the user test is complete; set `BRAWLER_NETWORK_TIMEOUT_SECONDS` for a bounded
session. The launcher runs through Cargo's target resolution, supervises every child, propagates
failures, and leaves no Brawler processes after shutdown. The individual Cargo commands remain
available for focused checks:

```sh
cargo fmt --all -- --check
cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings
cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings
cargo test --locked --no-default-features --features client --all-targets
cargo test --locked --no-default-features --features server --all-targets
cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1
cargo test --locked --no-default-features --features network-test --test performance -- --nocapture
cargo build --locked --no-default-features --features client --bin brawler-client
cargo build --locked --no-default-features --features server --bin brawler-server
cargo run --locked --no-default-features --features client --bin brawler-client
cargo run --locked --no-default-features --features server --bin brawler-server -- --bind 127.0.0.1:5000
./scripts/check-server-features.sh
just network
just network-combat
just network-controller
just network-combat-30
just network-combat-60
just network-combat-high
just network-smoke
just network-routed-smoke
just network-product-lobby
just network-product-lobby-smoke
just network-product-queue-smoke
just network-product-match
just network-product-match-3v3
just network-product-match-smoke
just network-product-match-3v3-smoke
just network-routed-ipv6-smoke
just network-routed-evidence
just network-routed-capture
just verify-routed-capture capture=target/routed-capture.pcap
just network-paired-evidence
just test-paired-evidence
just network-direct
just network-direct-smoke
just network-terrain
just network-terrain-hot-zone
just closeout-wipeout
just closeout-hot-zone
just prediction-comparison
just help
just check-routing
just test-routing
just clippy-routing
just check
just build
just test
just lint
just verify
just user-test
just docs
just clean
```

`just network` now launches the v2 routed topology at one public UDP address: a plain supervisor,
an isolated lobby worker, a match worker after allocation, and two clients. Use
`just network-routed-smoke` for the bounded headless lobby-to-match check. The completed v1 direct
UDP topology remains available as `just network-direct` and `just network-direct-smoke` until the
roadmap's M09 retirement gate.

Normal `brawler-client` startup now opens the product Title without connecting. Play opens Server
Select, whose first-run address is `127.0.0.1:5000`; the client connects to a supervisor-backed
product lobby and shows its advertised game types without allocating a match. Use
`just network-product-lobby` for the windowed playtest or `just network-product-lobby-smoke` for the
bounded two-client real-process check of that welcome boundary and the no-allocation guarantee.
Use `just network-product-match` or `just network-product-match-3v3` to open four or six product
windows against one local supervisor. Select Play in every window, then join the advertised game.
The corresponding `-smoke` commands drive the same match path headlessly.

`just network-routed-evidence` runs bounded cold routed-process cycles (five by default; use
`just network-routed-evidence <cycles> <timeout-seconds> <wipeout|hot-zone|both|crash-restart>`).
It records the exact
Result-driven worker lifecycle, per-role RSS, directional public/inner/IPC traffic, bounded routing
owner-loop latency diagnostics, final route/queue/drop counters, and process cleanup in
`target/routed-evidence-<UTC timestamp>.json`. The report explicitly marks paired CPU/direct
bandwidth, full IPC-to-worker latency, packet-only IPC overhead, packet-capture MTU, and paired
fixed-tick gates unsupported; correlated stop/reap and allocation-to-connected samples remain
below their required campaign cardinalities. It never fabricates those measurements. Add
`--keep-artifacts` by invoking `python3 scripts/network-routed-evidence.py --keep-artifacts` to
retain per-cycle logs.

`just network-routed-ipv6-smoke` runs the same production routed process check over `[::1]`; the
client derives an IPv6 local socket from the selected `--server` address, or accepts an explicit
`--local-addr`. On macOS, `just network-routed-capture capture=target/routed-capture.pcap` runs
both IPv4 and IPv6 headless smokes under `tcpdump` on `lo0` and parses the resulting classic pcap with
`scripts/verify-routed-capture.py`. BPF capture permission may require an approved administrator
session. No capture result is considered evidence unless a real pcap is produced and the parser
observes IPv4/IPv6 UDP payloads no larger than 1,200 bytes with no IPv4 fragmentation or IPv6
Fragment header; unavailable or malformed captures stay unsupported.

`just network-paired-evidence 1 90 wipeout` runs the bounded one-pair M01 comparison smoke; the
canonical gate is `just network-paired-evidence 3 90 wipeout` (or `hot-zone`). It runs the existing
direct and routed verification launchers sequentially on the same host, source tree, mode, and
verification rules, requires the exact expected process-role cardinalities, samples every Brawler process's CPU time and RSS, and writes
`target/paired-evidence-<UTC timestamp>.json`. Aggregate routed CPU must be no more than 20% over
the direct aggregate when both process series and a correlated common observation interval are
comparable. Direct server transport bytes are compared with routed supervisor match-worker inner
ingress/egress bytes independently, with a 10% limit per direction and for the total. Routed public-envelope and mixed packet/control IPC bytes are
reported as overhead diagnostics and are never compared with direct gameplay bytes. Missing
comparable samples or common-window checkpoints produce an explicit `unsupported` result. Run `just test-paired-evidence` for
the parser and threshold-gate tests without starting processes.

The server accepts `--bind`, `--max-clients`, and `--handshake-timeout-ms`. A normal windowed client
starts at the controller-friendly Title screen and does not connect; `--client-id` defaults to 1 in
that offline shell. `--auto-connect` selects the established development/network path and requires
an explicit `--client-id`. The client also accepts `--server`, `--local-addr`, and `--build-preset 1..5` (`1` Runner, `2` Bruiser, `3` Controller,
`4` Duelist, `5` the default legal custom Pulse), plus bounded automation flags `--headless --exit-after-roster 2
--move-axis X,Y --aim-axis X,Y --aim-dummy --fire --ultimate --simulation-ticks N`. `--combat-demo` enables the
same authoritative aim-at-dummy/fire loop in a windowed client for a reproducible visual smoke run.
Use `--window-size WIDTHxHEIGHT` to reproduce a supported visual-check layout.
On macOS, `scripts/macos-client-bundle.sh` creates a temporary addressable `.app` wrapper around the
already-built client for native visual automation; it prints the wrapper path and does not modify the
production application composition.
`--controller-demo` creates a synthetic gamepad only for the windowed controller-path smoke; it
still uses the normal gamepad sampler and native input buffer, but does not substitute for a
physical controller.
`RUST_LOG` controls log filtering, for example `RUST_LOG=brawler=info`. Window titles identify the two clients; structured logs report
connection outcome and stable `(player_id, network_entity_id)` roster entries. `just network-smoke`
is the routed two-client lobby-to-match check; `just network-direct-smoke` retains the v1
server-side movement/facing assertion.

For the supervised combat path, use `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ASSERT_COMBAT=1
./scripts/network.sh`. Its legacy combat verifier composes an explicit test-only dummy fixture and
waits for server-verified shots, hits, damage, defeat, reset, and both client observations; production
Wipeout composition has no practice dummy. Use `scripts/network-match.sh` for the current four-client
match, respawn, telemetry, and restart process gate. It defaults to the shortened verification rules;
set `BRAWLER_NETWORK_WIPEOUT_RULES=production`, raise `BRAWLER_NETWORK_SIMULATION_TICKS`, and set a
matching `BRAWLER_NETWORK_MATCH_TIMEOUT_SECONDS` for a controlled normal-duration comparison.
`BRAWLER_NETWORK_PROFILE=local|typical|adverse` applies the corresponding Lightyear receive
conditioner; `just network-combat-profiles` repeats all three profiles and reports median/p95
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
state, sentry health/lifetime, and cooldown/reload; fighters also show debug health bars.
The arena is reconstructed from the authoritative replicated map snapshot. Its perimeter and cover
block fighters and weapon delivery, while client sprites, audio, and HUD state remain presentation-only.
For a reproducible single-shooter visual combat pass, run `just network-combat`; it starts two
windowed clients, with client 1 using `--combat-demo` and client 2 idle. The demo uses the same native
input buffer while continuously aiming at and firing on the neutral dummy. To launch the processes
manually, start `brawler-server`, then run one client with `--client-id 1 --auto-connect --combat-demo` and the
second with `--client-id 2 --auto-connect`; enabling the demo flag on both clients intentionally produces one
projectile stream from each player toward the neutral dummy.

Repeat the same scenario at the milestone's render conditions with `just network-combat-30`,
`just network-combat-60`, and `just network-combat-high`. These select
`BRAWLER_RENDER_PROFILE=30|60|high`; the fixed authoritative simulation remains 60 Hz. The
high-refresh profile uses continuous updates and no-vsync presentation, while actual display refresh
and physical-controller behavior still require the target hardware.

For a focused live movement trace, run
`BRAWLER_INPUT_TRACE=1 RUST_LOG=brawler=info just run`. The trace reports focused-window WASD
sampling, the Lightyear input target, authoritative server movement, replicated interpolation
history, and the final client presentation pose only when those states materially change.

Do not use `--all-features` as a supported application build: client and server are independently tested production configurations, while `network-test` is the dedicated separate-app Crossbeam integration configuration. Cargo features are additive.

## Repository conventions

Authoritative authored gameplay data lives under `content/v1/` and is compiled into both roles.
Client-only runtime art/audio lives under `assets/brawler/`; exact source and CC0 provenance are
recorded in `assets/manifest.ron` with retained source license texts under `assets/licenses/`.
The active implementation scope is always the current milestone file; deferred release polish must
remain visible in the roadmap rather than being folded into unrelated v2 infrastructure work.
