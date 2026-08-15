# Brawler

Brawler is a server-authoritative top-down arena shooter. The current v1 slice includes typed weapon
and map recipes, four weapon profiles, a server-resolved Wipeout arena, replicated match/map/combat
state, and a client-only presentation shell over the Lightyear Netcode/UDP connection.

## Toolchain

The repository pins Rust 1.95.0 in [`rust-toolchain.toml`](rust-toolchain.toml). Bevy is pinned to 0.19.1 and Lightyear to 0.29.0. `Cargo.lock` is committed and must be updated intentionally.

## Canonical commands

Run these from the repository root:

```sh
just
```

`just` lists the development recipes. Use `just run` for one server and one client, `just network`
for one server and two distinguishable client windows, `just network-combat` for two windows with
client 1 automatically engaging its opponent, `just network-controller` for a synthetic controller-path
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
cargo run --locked --no-default-features --features client --bin brawler-client -- --client-id 1
cargo run --locked --no-default-features --features server --bin brawler-server -- --bind 127.0.0.1:5000
./scripts/check-server-features.sh
just network
just network-combat
just network-controller
just network-combat-30
just network-combat-60
just network-combat-high
just network-smoke
just help
just check
just build
just test
just lint
just verify
just user-test
just docs
just clean
```

The server accepts `--bind`, `--max-clients`, and `--handshake-timeout-ms`. The client accepts
`--server`, required `--client-id`, and `--weapon-preset 1..4` (`1` Pulse, `2` Scatter, `3` Arc,
`4` Blade), plus bounded automation flags `--headless --exit-after-roster 2
--move-axis X,Y --aim-axis X,Y --aim-dummy --fire --simulation-ticks N`. `--combat-demo` enables the
same authoritative aim-at-dummy/fire loop in a windowed client for a reproducible visual smoke run.
`--controller-demo` creates a synthetic gamepad only for the windowed controller-path smoke; it
still uses the normal gamepad sampler and native input buffer, but does not substitute for a
physical controller.
`RUST_LOG` controls log filtering, for example `RUST_LOG=brawler=info`. Window titles identify the two clients; structured logs report
connection outcome and stable `(player_id, network_entity_id)` roster entries. `just network-smoke`
also requires a server-side movement/facing assertion before it succeeds.

For the supervised combat path, use `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ASSERT_COMBAT=1
./scripts/network.sh`. Its legacy combat verifier composes an explicit test-only dummy fixture and
waits for server-verified shots, hits, damage, defeat, reset, and both client observations; production
Wipeout composition has no practice dummy. Use `scripts/network-match.sh` for the current four-client
match, respawn, telemetry, and restart process gate.
`BRAWLER_NETWORK_PROFILE=local|typical|adverse` applies the corresponding Lightyear receive
conditioner; `just network-combat-profiles` repeats all three profiles and reports median/p95
convergence timings.

The network launcher waits for its own server's readiness signal before starting clients, so a bind
collision cannot accidentally test a pre-existing server. It supervises the already-built binaries
directly, so Ctrl-C terminates the actual server/client processes before the launcher exits. It
remains running when one windowed client closes so the remaining roster can be observed. Restart
that client with the same individual command and `--client-id`; set
`BRAWLER_NETWORK_TIMEOUT_SECONDS` to add a bounded windowed-session deadline when needed.

Milestones 03–07 provide movement, combat, the first authored arena, and a complete Wipeout loop. At
weapon selection, use Left/Right or A/D and Space/Enter to confirm; on a controller use the D-pad or
left stick and South. In Waiting, Space/Enter or South readies the participant, and the same input
requests the next match after the completed-phase lock. During play, use WASD to move, mouse position
to aim, and mouse-left to fire; Q/E remain reserved for active item/ultimate inputs. A connected
controller uses the left stick for movement, right stick for aim, right trigger to fire, the other
trigger for reserved gameplay input, and Start for pause. Hold Tab or controller Select for the full
roster scoreboard. The HUD shows match phase, score/time/result, roster/weapon/readiness, respawn and
protection state, health, ammo, and cooldown/reload; fighters also show debug health bars.
The arena is reconstructed from the authoritative replicated map snapshot. Its perimeter and cover
block fighters and weapon delivery, while client sprites, audio, and HUD state remain presentation-only.
For a reproducible single-shooter visual combat pass, run `just network-combat`; it starts two
windowed clients, with client 1 using `--combat-demo` and client 2 idle. The demo uses the same native
input buffer while continuously aiming at and firing on the opposing fighter. To launch the processes
manually, start `brawler-server`, then run one client with `--client-id 1 --combat-demo` and the
second without `--combat-demo`; enabling the flag on both clients intentionally produces one
projectile stream from each player toward the dummy.

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
