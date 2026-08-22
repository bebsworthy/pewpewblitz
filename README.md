# PewPew Blitz

PewPew Blitz is a server-authoritative top-down arena shooter built around player-authored brawler
builds, readable combat, meaningful tradeoffs, and short objective matches. The current product
supports Wipeout and Hot Zone, bounded weapon/build recipes, abilities and passives, destructible
terrain, independently authored maps, a fixed-camera 3D presentation, server-local multiplayer,
and server-hosted practice matches with inert roster bots.

The windowed client presents the Player Dashboard and renders the world, HUD, audio, and local input.
The routed supervisor and lobby own admission and server-local queues; isolated match workers own
authoritative movement, combat, terrain, modes, lifecycle, and outcomes. Avian 2D remains the planar
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

One client is sufficient for Practice. Multiplayer game types form only their exact advertised
human roster, so the default First Blood path requires two clients.

## Canonical commands

Run `just` to list the supported everyday surface.

| Command | Purpose |
|---|---|
| `just server` | Start the routed supervisor and production lobby on localhost |
| `just client` | Open one interactive product client against the local server |
| `just run <N>` | Build once, start the routed server, and open exactly 1–16 interactive clients |
| `just fmt` | Format all Rust sources |
| `just check` | Check every independently buildable role |
| `just lint` | Run formatting, Clippy, server isolation, and renderer-boundary checks |
| `just test` | Run deterministic routing, client, server, network, and performance suites |
| `just e2e [2, 4, or 6]` | Run the real-process 1v1, 2v2, or 3v3 product path; default is 2 |
| `just v3-render-evidence [path]` | Record the bounded native release render report; the historical recipe name is retained |
| `just ci` | Run lint, deterministic tests, and the complete 2/4/6-client E2E matrix |
| `just clean` | Remove Cargo build artifacts |

E2E runs choose an unused loopback port by default and may run beside an interactive server. Set
`BRAWLER_ROUTED_BIND` only when a fixed test address is required.

## Current player flow

Dashboard is the sole authenticated home. Change Brawler and Change Game open child selection
surfaces with explicit Confirm and Back behavior. Play enters the selected game type's multiplayer
queue; Practice asks the connected server to allocate an authoritative match with inert `Bot N`
fighters filling the remaining roster. Neither action launches a server process from the client.

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

The match menu does not pause authoritative simulation. It suppresses local gameplay intent and
offers Resume, Settings, Scoreboard, and confirmed Leave Match while the match continues. Settings
opened from the product shell or match menu are validated and persisted locally; input calibration
and bindings shape local intent before quantization and never become server authority.

## Server configuration and authored content

Server game types live in [`config/server/game-types.ron`](config/server/game-types.ron). Each stable
advertisement combines one mode, compatible map pool, exact team topology, and flat bounded match
rules. Startup validates the catalog before the lobby advertises it or a match worker installs it.

Non-map authored gameplay catalogs live under [`content/v1/`](content/v1/). Built-in map documents
and shared server-safe map definitions live under [`content/v4/`](content/v4/). To add a built-in
map, create `content/v4/maps/builtin/<map-key>.ron` and add its stable metadata and admission revision
to the sorted [`content/v4/maps/index.ron`](content/v4/maps/index.ron). Startup rejects disagreement
between the index and embedded map sources.

Map recipes reference stable semantic IDs rather than client asset paths. The server lowers those
placements into a resolved authoritative snapshot. The client maps stable presentation IDs through
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
- `--build-preset 1..5`, movement/aim/fire flags, demos, screenshots, and render reports are
  development or automation inputs rather than alternate product flows;
- `BRAWLER_FORCE_PRIMITIVE_WORLD=1` verifies deterministic meshes inside the sole 3D renderer; it is
  not a renderer or content-mode selector;
- `RUST_LOG` controls filtering, for example `RUST_LOG=brawler=info`.

Do not use `--all-features` as a supported application build. Cargo features are additive: client
and server are independently tested production configurations, while `network-test` is the
dedicated separate-App integration-test configuration.

## Verification and diagnostics

`just test`, `just e2e`, and `just ci` are the canonical automated gates. For a focused live movement
trace, run:

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
