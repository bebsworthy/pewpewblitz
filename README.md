# PewPew Blitz

PewPew Blitz is a server-authoritative top-down arena shooter built around player-authored brawler
builds, readable combat, meaningful tradeoffs, and short objective matches. The current product
supports Wipeout, Hot Zone, and Heist; bounded weapon/build recipes; abilities and passives;
destructible and health-bearing map assets; treasure-chest restoration pickups; the consolidated
Feature Yard test-map family; a fixed-camera 3D presentation; server-local multiplayer; and server-
hosted Practice matches with active deterministic Pulse/Dash bots.

The windowed client presents the Player Dashboard and renders the world, HUD, audio, and local input.
The routed supervisor and lobby own admission and server-local queues; isolated match workers own
authoritative movement, combat, map dynamics, modes, lifecycle, and outcomes. Avian 2D remains the planar
collision authority beneath the 3D presentation.

The product is named PewPew Blitz; repository, crate, executable, log-target, and environment-variable
identifiers retain the established `brawler` name.

Start with the [documentation index](docs/README.md) for durable product and technical
specifications, the [product direction](docs/00-product-direction.md) for the vision, and the
[candidate index](docs/backlog.md) for unresolved future work. Version roadmaps under
`docs/implementation/` preserve delivery history and evidence.

## Requirements and toolchain

- Rust 1.95.0, pinned by [`rust-toolchain.toml`](rust-toolchain.toml), with Rustfmt and Clippy;
- [`just`](https://github.com/casey/just) for the canonical development commands;
- macOS for the current native client-development and visual-validation path;
- Python 3 for E2E port selection and the advanced network evidence scripts.

Bevy is pinned to 0.19.1 and Lightyear to 0.29.0. `Cargo.lock` is committed and dependency updates
must be intentional.

## Quick start

Start the routed server and two interactive clients together:

```sh
just run 2
```

Or use separate terminals:

```sh
just server
```

```sh
just client
```

The server listens on `127.0.0.1:5000`. Normal client startup tries an explicit `--server`, then the
most recently successful server, then the local default. A successful connection opens the Player
Dashboard; cancellation or bounded connection failure opens Server Select. Press Ctrl-C in a
`just run` session to stop its complete local process tree.

`just run N` gives each local window a stable numbered client-data slot under
`target/dev/clients/1..N` and reuses the logical server under `target/dev/server`. Repeating the
command therefore preserves N independent local identities without concurrent writes to one
`connections.ron`. `just client` continues to use the normal platform user-data location. Set
`BRAWLER_DEV_DATA_DIR` to relocate all development slots, or `BRAWLER_CLIENT_DATA_DIR` to give one
manually launched client an explicit data directory.

One client is sufficient for Practice. Multiplayer game types form only their exact advertised
human roster; the default Feature Yard Wipeout 2v2 path therefore requires four clients. The
catalog exposes exact 1v1, 2v2, and 3v3 Wipeout, Hot Zone, and Heist entries. The 1v1/2v2 paths use
Feature Yard; 3v3 uses Verdant Crossfire, Switchback Basin, and Powderline Vault respectively.

For the development-only Balance Lab, start the tuning-enabled routed server and one interactive
client, then use that client to enter Practice:

```sh
just balance-lab
```

The launcher opens <http://127.0.0.1:5123> in the default browser immediately. The endpoint does not
exist until the client enters Practice, so the launcher opens the URL again as soon as that worker
is ready. Keep the working page open while returning to the menu; it waits for and reconnects to the
next Practice worker. Accepted overrides are validated and persisted under
`target/balance-lab/session-v2.json`, and **Restore canonical defaults** removes that local override
without changing canonical authored content. The page also shows the authoritative human/bot roster,
teams, admitted fighter/weapon/ability choices, and effective weapon-part modifiers for the current
Practice worker.
See the [Balance Lab operator and maintenance guide](./docs/15-balance-lab.md) for validation rules,
limitations, and the required checklist when fighter or weapon properties change.

## Canonical commands

Run `just` to list the supported everyday surface.

| Command | Purpose |
|---|---|
| `just server` | Start the routed supervisor and production lobby on localhost |
| `just balance-lab` | Start Balance Lab, one interactive client, and its default-browser page |
| `just client` | Open one interactive product client against the local server |
| `just run <N>` | Build once, start the routed server, and open exactly 1–16 interactive clients |
| `just fmt` | Format all Rust sources |
| `just check` | Check every independently buildable role |
| `just lint` | Run formatting, Clippy, server isolation, and renderer-boundary checks |
| `just test` | Run deterministic routing, client, server, network, and performance suites |
| `just e2e [2, 4, or 6]` | Run the real-process 1v1, 2v2, or 3v3 product path; default is 2 |
| `just practice-e2e [game type]` | Run one exact real-process Practice match with one human and server-filled bots; default is `wipeout-1v1` |
| `just v3-render-evidence [path]` | Record the bounded native release render report; the historical recipe name is retained |
| `just ci` | Run lint, deterministic tests, the complete 2/4/6-client E2E matrix, and all nine Practice game types |
| `just clean` | Remove Cargo build artifacts |

E2E runs choose an unused loopback port by default and may run beside an interactive server. Set
`BRAWLER_ROUTED_BIND` only when a fixed test address is required.

## Current player flow

Dashboard is the sole authenticated home. The server loads the account's saved-brawler profile
before admitting the client. The accepted lobby response atomically supplies that profile and the
server's bounded, revisioned brawler catalog: legal fighter profiles, weapon bases, ultimates,
passives, display metadata, preview values, and selection limits. The client installs that catalog
for the connection, drives every brawler screen from it, and clears it on disconnect or server
change; it does not reconstruct legal choices from numeric ranges or local name tables.

New profiles start empty; create a brawler before Play or Practice. Each brawler permanently binds
one advertised fighter profile and weapon base, while its name, ultimate, two passives, and four
generic weapon-part slots remain server-owned editable data outside queue. Every profile receives
eight starter part instances once. Tapping the Dashboard brawler card opens the full-screen
Brawlers List, then a full-screen Brawler screen; creation, ability customization, and weapon
customization are full-screen child destinations. Delete is a small contextual confirmation over
the invoking Brawler screen. Play enters the selected game type's multiplayer queue and Practice
fills the remaining roster with active deterministic `Bot N` fighters using the canonical
Pulse/Dash recipe. Queue admission freezes the selected
brawler revision and resolved part modifiers for that match.

Queue cancellation, loading cancellation, confirmed leave, and ordinary no-result return converge
on Dashboard while the lobby remains valid. Results retains the authoritative outcome and offers
Dashboard plus exact replay when the previous game-type ID still exists in the fresh lobby catalog.

The Dashboard uses its Wide composition when effective UI space is at least `1000x640` and the same
semantic actions in a vertically scrollable Compact composition below either threshold. Effective
size is logical window size divided by persisted UI scale. Disabled actions are skipped, focused
Compact actions scroll into view, and Reduced Motion or Reduced Effects freezes non-essential
procedural motion without removing state feedback.

The complete navigation, recovery, and accessibility contract lives in the
[player experience specification](docs/13-player-ux.md).

## Controls

### Product shell

- Mouse hover/click, keyboard arrows or WASD, and controller D-pad navigate the same actions.
- Enter or Space and controller South confirm or activate.
- Escape and controller East cancel or return to the nearest valid parent.
- Direct address and optional display-name fields accept keyboard input and paste; generated names
  and saved servers keep ordinary controller use independent from text entry.

### Match

| Action | Keyboard and mouse | Controller |
|---|---|---|
| Move | WASD | Left stick |
| Aim | Mouse position | Right stick |
| Primary fire | Left mouse button | Right trigger |
| Ultimate | E | Right bumper |
| Active item | Q, reserved until a supported active-item capability exists | Left trigger, reserved |
| Match menu | Escape | Start |
| Hold scoreboard | Tab | Select |
| Capture frame + client state | F12 | North face button |

The match menu does not pause authoritative simulation. It suppresses local gameplay intent and
offers Resume, Settings, Scoreboard, and confirmed Leave Match while the match continues. Settings
opened from the product shell or match menu are validated and persisted locally; input calibration
and bindings shape local intent before quantization and never become server authority.

Evidence captures are saved as matching PNG/JSON pairs under the platform Pictures directory at
`PewPew Blitz/Captures` (or the application data directory when Pictures is unavailable).
`BRAWLER_CAPTURE_DIR` overrides that destination for troubleshooting and automation.

Canonical fighters recover `10` health per second after `3.0` seconds without a server-accepted
player attack. Current weapons recover one round or charge every `1.3` seconds; firing consumes
stock without resetting an interval already in progress. Both mechanics remain server-authoritative.
See the [fighter specification](docs/02-fighter-model.md) and
[weapons and abilities specification](docs/03-weapons-and-abilities.md) for the exact lifecycle and
replication rules.

## Server configuration and authored content

Server game types live in [`config/server/game-types.ron`](config/server/game-types.ron). Each stable
advertisement combines one mode, compatible map pool, exact team topology, and flat bounded match
rules. The same operator catalog declares the admitted minimum and maximum map width and height;
the checked-in server accepts `20×20` through `512×512` cells. Startup validates the envelope and
every advertised map before the lobby advertises it or a match worker installs it.

Placement capacity follows the map area and four mutually exclusive asset slots rather than a
style-specific count. A `512×512` map may therefore conceal all 262,144 cells and may resolve up to
1,048,576 total slot placements. Resolved snapshots are bounded at 32 MiB; damageable and other
mutable object families retain their tighter independent limits. Extreme-density rendering and
lookup optimization are deferred until measured content requires them.

The product catalog currently presents one shared **Feature Yard** integration-map family through
separate Wipeout, Hot Zone, and Heist recipes. Those recipes intentionally reuse identical geometry
while retaining mode-specific validation and do not claim release-map balance or fun.

Build-embedded, headless-safe gameplay definitions live under
[`content/catalogs/`](content/catalogs/). Built-in sparse-grid map documents live under
[`content/maps/`](content/maps/). To add a built-in map, create
`content/maps/builtin/<map-key>.ron` and add its stable metadata and admission revision to the
sorted [`content/maps/index.ron`](content/maps/index.ron). Startup rejects disagreement between the
index and embedded map sources.

Map recipes reference stable `MapAssetId`s rather than client asset paths. The server derives
surfaces, collision, destruction, damageable-object durability, spawns, and typed mode anchors from
shared gameplay profiles and lowers them into a resolved authoritative snapshot. Runtime barrels,
chests, restoration pickups, and Heist objectives remain server-owned and replicate stable current
state; the client maps stable visual IDs through
[`assets/catalogs/`](assets/catalogs/) to art under [`assets/brawler/`](assets/brawler/). Exact source
and CC0 provenance live in [`assets/manifest.ron`](assets/manifest.ron), with retained license texts
under [`assets/licenses/`](assets/licenses/).

## Executable configuration

Run the built `brawler-client` or `brawler-server` binary with `--help` for its complete bounded CLI
contract. Important ownership rules are:

- the interactive product shell uses routed UDP, generates a random nonzero client ID, and rejects
  an explicit `--client-id` or `--local-addr`;
- `--auto-connect`, headless clients, and demo automation require an explicit `--client-id`;
- `--server` prefills the interactive connection target and selects the automation endpoint;
- `--window-size WIDTHxHEIGHT` reproduces supported visual-check layouts;
- `--weapon-preset 1..4`, movement/aim/fire flags, demos, screenshots, and render reports are
  development or automation inputs rather than alternate product flows;
- `BRAWLER_FORCE_PRIMITIVE_WORLD=1` verifies deterministic meshes inside the sole 3D renderer; it is
  not a renderer or content-mode selector;
- `RUST_LOG` controls filtering, for example `RUST_LOG=brawler=info`.

## Profile data and backup

The routed supervisor keeps its stable logical-server ID and lobby-owned `profiles.sqlite3` under
its data directory (`target/dev/server` for `just server` and `just run`). The database uses SQLite
WAL, foreign keys, transactions, an application ID, and forward-only schema versions. Startup
rejects corruption or an incompatible schema and never replaces owned data with an empty profile.

Create a stopped-server backup with SQLite's online backup API:

```sh
cargo run --locked --no-default-features --features server \
  --bin brawler-profile-admin -- backup \
  --database target/dev/server/profiles.sqlite3 \
  --output target/dev/server/profiles-backup.sqlite3
```

The command refuses to overwrite its output and validates the completed copy. To restore, stop the
server, preserve the current database together with any `-wal` and `-shm` siblings, copy the
validated backup into a fresh data directory as `profiles.sqlite3`, and restart the supervisor.

Do not use `--all-features` as a supported application build. Cargo features are additive: client
and server are independently tested production configurations, while `network-test` is the
dedicated separate-App integration-test configuration.

## Verification and diagnostics

`just test`, `just e2e`, `just practice-e2e`, and `just ci` are the canonical automated gates. For a
focused live movement trace, run:

```sh
BRAWLER_INPUT_TRACE=1 RUST_LOG=brawler=info just run 1
```

The trace reports input sampling, the Lightyear target, authoritative movement, interpolation, and
final presentation only when those states materially change.

Advanced scripts remain available for focused evidence and historical comparison:

| Script | Role |
|---|---|
| [`scripts/v3-render-evidence.sh`](scripts/v3-render-evidence.sh) | Bounded routed native render report used by the retained `just v3-render-evidence` recipe |
| [`scripts/network-routed-evidence.py`](scripts/network-routed-evidence.py) | Cold routed worker lifecycle, resource, traffic, latency, and cleanup evidence |
| [`scripts/network-routed.sh`](scripts/network-routed.sh) | Focused production routed-process smoke, including explicit IPv6 runs |
| [`scripts/network-routed-capture.sh`](scripts/network-routed-capture.sh) | macOS loopback packet capture and MTU/fragment verification |
| [`scripts/network-paired-evidence.py`](scripts/network-paired-evidence.py) | Historical direct-versus-routed comparison campaign |
| [`scripts/network.sh`](scripts/network.sh) | Retained legacy direct-UDP diagnostic baseline |
| [`scripts/network-match.sh`](scripts/network-match.sh) and [`scripts/network-combat-profiles.sh`](scripts/network-combat-profiles.sh) | Legacy direct-UDP match and impairment gates |
| [`scripts/macos-client-bundle.sh`](scripts/macos-client-bundle.sh) | Temporary addressable `.app` wrapper for native visual automation |

These scripts must report unsupported or unavailable evidence honestly. Their historical thresholds,
campaign cardinalities, environment variables, and accepted results remain in the owning version
milestones and script tests. Legacy direct-UDP behavior does not define the product shell, current
match flow, or production hosting topology.

## Repository conventions

- Bevy's `World` is the runtime gameplay model. Authored definitions, selected builds, resolved
  match data, mutable ECS state, protocol registration, telemetry, and presentation remain distinct.
- Dedicated-server builds exclude rendering, windowing, audio, device input, and client assets.
- Stable protocol and content IDs cross process boundaries; process-local ECS `Entity` identity does
  not.
- The current implementation scope, when a version is active, is the next validated milestone file.
  Deferred work remains in the [canonical candidate index](docs/backlog.md) or the owning version
  backlog.
- Checked-in upstream references under `references/` are read-only implementation material unless a
  snapshot update is explicitly requested.
