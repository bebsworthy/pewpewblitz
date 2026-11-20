set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# List the everyday development commands.
help:
    @just --list

# Run the routed supervisor and production lobby on localhost.
server:
    ./scripts/dev.sh server

# Run the Practice Balance Lab topology with one interactive client.
balance-lab:
    ./scripts/dev.sh balance-lab

# Open one interactive product client against the local routed server.
client:
    ./scripts/dev.sh client

# Open the development-only read-only production VFX viewer.
# Native production-control review surface (no server needed).
ui-gallery:
    cargo run --locked -p brawler-client --features automation --bin brawler-ui-gallery

vfx-lab args="":
    cargo run --locked -p brawler-client --features automation --bin brawler-vfx-lab -- {{args}}

# Run the routed server and exactly N interactive clients.
run clients:
    ./scripts/dev.sh run {{clients}}

# Build the Android player APK (arm64-v8a); optional HOST:PORT bakes an editable server prefill.
android-build server="":
    ./scripts/android.sh build {{server}}

# Build, install over adb, and launch the Android player APK on the connected device.
android-install server="":
    ./scripts/android.sh install {{server}}

# Build the upload-key-signed Play bundle (.aab); fails early without the upload key (README).
android-bundle server="":
    ./scripts/android.sh bundle {{server}}

# Format all Rust sources.
fmt:
    cargo fmt --all

# Check every independently buildable role.
check: _check-routing _check-client _check-automation _check-server _check-network _check-balance-lab

# Run formatting, Clippy, dedicated-server isolation, and player-client automation-exclusion checks.
lint: _fmt-check _balance-lab-web _clippy-routing _clippy-client _clippy-automation _clippy-server _clippy-network _clippy-balance-lab _clippy-network-balance-lab _server-features _client-features _v3-world-presentation _map-cleanup _lint-exceptions _e2e-config

# Run all deterministic Rust test suites, including the performance gates.
test: _test-routing _test-client _test-server _test-balance-lab _test-network _test-performance

# Run a real-process product match with 2, 4, or 6 clients (default: 2).
e2e clients="2":
    ./scripts/e2e.sh {{clients}}

# Run one exact two- or three-human Zombie cooperative roster through the routed product path.
zombie-e2e players="2":
    ./scripts/e2e-zombie.sh {{players}}

# Run one exact real-process Practice game with one human and manifest bots.
practice-e2e game_type="wipeout-1v1":
    ./scripts/e2e-practice.sh {{game_type}}

# Publish a map from one client, approve it with the operator CLI, and play it in a routed 1v1 queue match.
published-map-e2e:
    ./scripts/e2e-published-map.sh

# Keep one routed Practice match session alive across two 4 s outages: to a new client source port, then back to the original.
network-change-e2e game_type="wipeout-1v1":
    ./scripts/e2e-network-change.sh {{game_type}}

# Reach the lobby welcome on the first attempt through a proxy that delays every supervisor-to-client datagram.
slow-link-e2e delay_ms="1500":
    ./scripts/e2e-slow-link.sh {{delay_ms}}

# Lose one routed Practice match session beyond the 10 s session bound, then rejoin the same fighter in the still-running match.
rejoin-e2e game_type="wipeout-1v1":
    ./scripts/e2e-rejoin.sh {{game_type}}

# Complete a PvP 1v1 match by forfeit while one player's seat is held, then require that player to rejoin it for its Results.
completed-rejoin-e2e game_type="wipeout-1v1":
    ./scripts/e2e-completed-rejoin.sh {{game_type}}

# Record one bounded routed release-client render report plus paired PNG/JSON captures at 1280x720.
v3-render-evidence report="target/v3-render-evidence.txt":
    ./scripts/v3-render-evidence.sh {{report}}

# Capture player-shell screens natively (PNG + state JSON) under target/shell-capture/<size>/; route and size default to all.
shell-capture route="all" size="all":
    ./scripts/shell-capture.sh {{route}} {{size}}

# Rebuild the per-family fighter weapon GLBs, source master, and review sheet (headless). Rebuild
# Riff afterwards: his two-handed clips read the weapons' support-grip anchors.
weapon-assets blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/weapons/build_weapons.py

# Rebuild Riff's source master, review renders, and runtime GLB with Blender 5.2 LTS (headless).
riff-asset blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/characters/riff/build_riff.py

# Rebuild Bumper's source master, review sheets, and runtime GLB (headless). Like Riff, his
# two-handed clips read the weapons' support-grip anchors, and his build reuses Riff's builder.
bumper-asset blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/characters/bumper/build_bumper.py

# Rebuild Torque's runtime GLB, source master, and review renders (headless).
torque-asset blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/characters/torque/build_torque.py

# Rebuild Cobble's runtime GLB, source master, and review renders (headless).
cobble-asset blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/characters/cobble/build_cobble.py

# Rebuild Plunk's runtime GLB, source master, and review renders (headless).
plunk-asset blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/characters/plunk/build_plunk.py

# Rebuild Pollen's runtime GLB, source master, and review renders (headless).
pollen-asset blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/characters/pollen/build_pollen.py

# Rebuild Orbit's runtime GLB, source master, and review renders (headless).
orbit-asset blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/characters/orbit/build_orbit.py

# Rebuild Sumi's runtime GLB, source master, and review renders (headless).
sumi-asset blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/characters/sumi/build_sumi.py

# Rebuild the Crossroads Facility environment kit GLBs, source master, and review sheet (headless).
facility-kit blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/environment/crossroads-facility/build_kit.py

# Rebuild the Tidal Garden environment kit GLBs, source master, and review sheet (headless).
tidal-kit blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/environment/tidal-garden/build_kit.py

# Rebuild the Ashen Court environment kit GLBs, source master, and review sheet (headless).
ashen-kit blender="/Applications/Blender.app/Contents/MacOS/Blender":
    {{blender}} -b --factory-startup -P asset_src/environment/ashen-court/build_kit.py

# Run the complete automated gate, including 2/4/6-client product matches and all Practice types.
ci: lint test _e2e-matrix _practice-e2e-matrix

# Remove Cargo build artifacts.
clean:
    cargo clean

# Prune stale target/ artifacts and brl*/agent-* scratch dirs older than N days (default 2); `just sweep dry` only prints.
sweep *args:
    python3 scripts/sweep.py {{args}}

_fmt-check:
    cargo fmt --all -- --check

# The client role is the shared crate's `client` feature plus the `brawler-client` package (BRL-0170).
_check-client:
    cargo check --locked -p brawler -p brawler-client --no-default-features --features brawler/client --all-targets

_check-automation:
    cargo check --locked -p brawler-client --features automation --all-targets

_check-server:
    cargo check --locked --no-default-features --features server --all-targets

_check-network:
    cargo check --locked -p brawler -p brawler-client --no-default-features --features brawler/network-test,brawler-client/network-test --tests

_check-routing:
    cargo check --locked -p brawler-routing --all-targets

_balance-lab-web:
    npm ci --prefix tools/balance-lab-web
    npm run --prefix tools/balance-lab-web test
    npm run --prefix tools/balance-lab-web build

_check-balance-lab: _balance-lab-web
    cargo check --locked --no-default-features --features balance-lab --all-targets

_clippy-routing:
    cargo clippy --locked -p brawler-routing --all-targets -- -D warnings

_clippy-client:
    cargo clippy --locked -p brawler -p brawler-client --no-default-features --features brawler/client --all-targets -- -D warnings

_clippy-automation:
    cargo clippy --locked -p brawler-client --features automation --all-targets -- -D warnings

_clippy-server:
    cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings

_clippy-network:
    cargo clippy --locked -p brawler -p brawler-client --no-default-features --features brawler/network-test,brawler-client/network-test --tests -- -D warnings

_clippy-balance-lab:
    cargo clippy --locked --no-default-features --features balance-lab --all-targets -- -D warnings

_clippy-network-balance-lab:
    cargo clippy --locked -p brawler-client --features network-test,balance-lab --tests -- -D warnings

_server-features:
    ./scripts/check-server-features.sh

_client-features:
    ./scripts/check-client-features.sh

_v3-world-presentation:
    ./scripts/check-v3-world-presentation.sh

_map-cleanup:
    ./scripts/check-v8-map-cleanup.sh

_lint-exceptions:
    ./scripts/check-lint-exceptions.sh

_e2e-config:
    ./scripts/check-e2e-config.sh

# Client unit tests run with automation so the scripted-driver tests run beside player tests.
_test-client:
    cargo test --locked -p brawler -p brawler-client --no-default-features --features brawler/client,brawler-client/automation --all-targets

_test-server:
    cargo test --locked --no-default-features --features server --all-targets

_test-balance-lab:
    cargo test --locked --no-default-features --features balance-lab --all-targets
    cargo test --locked -p brawler-client --features network-test,balance-lab --test network builds::revised_catalog_loadout_keeps_build_identity_and_replicates_authoritative_values -- --test-threads=1

_test-network:
    cargo test --locked -p brawler-client --features network-test --test network -- --test-threads=1

_test-routing:
    cargo test --locked -p brawler-routing

_test-performance:
    cargo test --locked -p brawler-client --features network-test --test performance -- --nocapture

_e2e-matrix:
    ./scripts/e2e.sh 2
    ./scripts/e2e.sh 4
    ./scripts/e2e.sh 6

_practice-e2e-matrix:
    ./scripts/e2e-practice.sh wipeout-1v1
    ./scripts/e2e-practice.sh wipeout-2v2
    ./scripts/e2e-practice.sh wipeout-3v3
    ./scripts/e2e-practice.sh hot-zone-1v1
    ./scripts/e2e-practice.sh hot-zone-2v2
    ./scripts/e2e-practice.sh hot-zone-3v3
    ./scripts/e2e-practice.sh heist-1v1
    ./scripts/e2e-practice.sh heist-2v2
    ./scripts/e2e-practice.sh heist-3v3
