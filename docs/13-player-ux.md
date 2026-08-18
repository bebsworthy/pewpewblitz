# Player UX and server-local matchmaking design

## Purpose and scope

This document defines the proposed v2 product direction, pending user validation through the v2
roadmap and per-milestone specification reviews. It connects the current prototype to a
product-quality client shell, direct-connect multiplayer, server-local game queues, automatic match
formation, concurrent authoritative match workers, and a dependable bot-practice path. It is a
feature and architecture boundary, not a milestone specification: every v2 milestone still begins
with its own research and user-validated technical specification.

In this document, **matchmaking** means direct-connect, server-local, skill-free queueing: the
player first joins a known server, selects one of its game types, and the server forms matches from
that game type's pool. V2 does not include global or cross-server matchmaking, ELO/rank, parties,
accounts, authentication, or production backend services.

Status triage used below:

- **Have** — exists in the prototype and is usable or reusable.
- **Debug-only** — exists as development presentation that must become product UI.
- **Missing** — required for the agreed experience and not built.
- **Research required** — the product behavior is agreed, but the responsible milestone must
  research and validate the technical architecture before implementation.
- **Deferred** — explicitly outside v2.

## V2 feature boundary (proposed)

V2 includes:

1. A product-quality client shell with controller-first and keyboard/mouse navigation, settings,
   credits, recoverable errors, and local persistence.
2. Direct connection by address, explicit favorites, recent servers, session-scoped display names,
   and server-advertised game types.
3. A product build editor backed by the existing bounded recipe and server validation, plus a
   schema-versioned local last-used build.
4. Exact-topology, server-local queues with immutable queue tickets, deterministic team assignment,
   overflow handling, match reservation, loading/check-in, and one authoritative countdown.
5. Concurrent authoritative matches as isolated worker processes behind one logical server and one
   public UDP endpoint, with supervisor-owned admission, routing, cleanup, recovery, and measured
   host admission ceilings.
6. Exact 2v2 and 3v3 Wipeout and Hot Zone game types, subject to mode/map validation and each
   milestone's R&D.
7. A redesigned combat HUD, scoreboard, in-match menu, results flow, and non-fatal session
   lifecycle.
8. An authoritative bot-practice path so a first-time player can test controls and builds without a
   populated server queue.
9. An accessibility and usability baseline: input calibration/remapping, non-color-only team
   identification, UI scaling, reduced shake/flashes, and audio/display settings.
10. Automated lifecycle, race, cross-match isolation, recovery, performance, visual-layout, and
    controller-usability verification.

V2 explicitly defers:

- global or cross-server matchmaking, skill rating, rank, and leaderboards;
- public server registry/discovery and internet reachability work such as NAT traversal and relays;
- parties, invitations, private groups, and team-affinity guarantees;
- accounts, authentication, cloud saves, persistent arsenals, inventories, and entitlements;
- join-in-progress, match-session resumption, host migration, and spectator mode;
- production moderation, fleet orchestration, and public-server administration;
- a sideband REST control service;
- the complete production-art pipeline from docs 11–12. V2 includes the UI/HUD visual system and
  combat-readability polish, not the full terrain-theme, character-rig, skin, and VFX replacement.

## Hosting topology and matchmaking decisions

### Dedicated-server authority

Authoritative simulation always runs in a `brawler-server` worker configuration. One logical server
is a thin supervisor/router plus a long-lived lobby worker and isolated match worker processes. A
product practice launcher may start the same supervisor and worker topology locally, but the
windowed client never owns gameplay authority or bypasses the routed network, validation, and
server-owned simulation path. The proposed topology and its remaining R&D gates are recorded in
[Multi-process server and single-port UDP/IPC transport](./14-multiplayer-server-architecture.md).

### Joining and discovery

- Joining by `IP:port` or `hostname(:port)` is always available. A missing port uses a documented
  default, and the address is validated before a network attempt.
- Favorites are an explicit player action, stored in a schema-versioned local file as stable
  `name`/`addr` entries. Successfully joined addresses may enter a separate bounded recent-server
  list; joining does not silently create a favorite.
- Public server registry/discovery is deferred with internet reachability. A future registry may
  receive server heartbeats and populate a browse tab, but it is not part of v2 matchmaking and
  does not by itself solve NAT or relay requirements.

### Server-owned game types

The server operator defines a bounded immutable list of game types at process startup. Each game
type has a stable `GameTypeId` and configuration revision distinct from its display name, and binds
one mode to compatible maps, exact team topology, and validated settings:

```toml
[[game_types]]
id = "wipeout-2v2"
name = "Wipeout 2v2"
mode = "wipeout"
maps = ["arena"]
teams = 2
players_per_team = 2
rules = { target_score = 10 }

[[game_types]]
id = "hot-zone-3v3"
name = "Hot Zone 3v3"
mode = "hot-zone"
maps = ["hot_zone_arena"]
teams = 2
players_per_team = 3
```

Startup validation proves mode/rules compatibility, map anchors and spawn capacity, formation size,
content identities/revisions, and process ceilings before the server advertises a game type. The
server selects among multiple allowed maps by a deterministic rotation policy specified during the
owning milestone; clients do not vote or author rules in v2. Runtime hot-editing is deferred.

### Server-local pool formation

- Each advertised game type owns a FIFO pool of validated queue tickets.
- A queue ticket records the stable player/session identity, game type and revision, server-accepted
  immutable build snapshot, admission order, and a unique ticket/request identity.
- Formation requires the game type's exact topology. Lifecycle minimums handle later departures or
  forfeits; they are not permission to start an undersized match.
- The server reserves the oldest exact set of valid tickets, assigns teams through a deterministic
  server-owned rule, allocates a match instance, and moves those clients into match loading.
- Overflow tickets remain queued for the next formation.
- A disconnected or explicitly cancelled ticket is removed. Queue commands are idempotent and
  formation resolves cancellation races deterministically.
- Parties and group affinity are absent. Two friends may join the same queue, but v2 does not
  promise the same match or team.
- PvP game types never insert bots silently. Practice and any bot-filled game types are separately
  named and visibly identified.

### Concurrent matches as isolated workers

Concurrent matches are a v2 capability and a mandatory R&D gate, not an assumed
extension of the current single-match server. Each active match runs the existing authoritative
gameplay composition in its own OS process and Bevy `App`/`World`. The supervisor owns the single
public UDP socket, bounded routing and admission state, worker process lifecycle, and control/packet
IPC. It does not own combat ECS state or decode and reproduce Lightyear replication.

The client uses a fresh Lightyear connection when moving between lobby and match authority even
though both connections use the same public address. A short-lived opaque routing capability directs
the new handshake to the assigned worker. V2 does not attempt to migrate a live Lightyear/Netcode
session between processes.

The worker architecture reuses the existing authoritative server machinery rather than creating a
parallel gameplay implementation. Process isolation scopes fighters, projectiles, abilities, bots,
objectives, cues, rules, mode state, maps, terrain, physics, recovery, telemetry, and cleanup.
Shared immutable catalogs remain build inputs in every worker; mutable match state stays inside the
owning worker world. Host-wide capacity belongs to the supervisor; queue and reservation state
belong to the lobby worker.

## Current prototype state

The windowed client connects immediately on launch and never shows a product menu:

- **Auto-connect on boot.** Address, client id, build preset, and server mode are command-line
  configuration. There is no title, server selection, or game selection screen.
- **Join lifecycle is terminal.** Connecting leads to Active, Rejected, or Disconnected; rejection
  and disconnect exit the application instead of returning to a usable screen.
- **Build selection is a debug overlay.** Four presets and one bounded custom recipe already use
  server validation and the 12-point budget, but selection is attached to the waiting-match fighter
  rather than a pre-match session or queue ticket.
- **The waiting phase acts as a lobby.** All participants ready up, one process-global match starts,
  and restart quorum repeats the same roster.
- **HUD and results are debugging surfaces.** They expose correct match, roster, readiness, and
  outcome facts as text rather than product presentation.
- **Identity is numeric.** No display name exists.
- **Settings and credits lack product UI and persistence.** M11 supplies input-setting data and a
  debug overlay, not the complete v2 shell.
- **Headless automation exists and must survive.** Tests and demos drive the same authoritative
  join and gameplay path without pretending to operate UI.

## Experience principles

1. **Boot to a useful choice; fight within a minute when a path is available.** Practice is always
   available; a reachable populated favorite is a short path to PvP. The UI never invents a short
   queue estimate when the population cannot support one.
2. **Controller-first navigation, keyboard/mouse first-class.** Focus navigation, confirm, cancel,
   and back are consistent. Direct address and optional name editing may use keyboard/paste;
   controller users can accept a generated name and navigate saved servers without text entry.
3. **One place for every choice.** Game type and build are selected before queue admission. The
   build is locked while queued; changing it means cancelling or completing an acknowledged ticket
   update before formation.
4. **Queue state is honest and private.** Show game type, `queued/needed`, the player's accepted
   build, and formation/loading progress. Do not publish every waiting player's name or build.
5. **Fail soft.** Rejection, timeout, mismatch, unavailable content, cancellation races, and
   disconnect lead to a clear recoverable state. Headless automation retains deterministic exit
   codes.
6. **Nothing authoritative moves to the client.** UI sends bounded intent; the server validates
   membership, builds, formation, teams, maps, lifecycle, and outcomes.
7. **Automation follows the product path.** Headless clients bypass presentation but use the same
   session, queue, formation, loading, and match protocols.
8. **Readable competition is accessible.** Team identity is never color-only, important state has
   shape/text/icon support, and settings can reduce presentation that impairs play.

## Client flow and overlays

One flat state enum cannot model a top-level flow plus Settings, scoreboard, confirmation, and
error overlays. The client owns two presentation layers:

- `ClientFlow`: `Title`, `ServerSelect`, `Connecting`, `GameSelect`, `Queue`, `MatchLoading`,
  `Match`, and `Results`.
- `ClientOverlay`: `None`, `BuildEditor`, `Settings`, `Credits`, `InMatchMenu`, `Scoreboard`,
  `Confirmation`, and `Error`.

The milestone R&D may select exact Bevy state/resource types, but it must preserve this separation,
an explicit return destination for overlays, deterministic focus restoration, and complete cleanup
on flow changes.

### There is no pause in multiplayer

The in-match menu never pauses the authoritative match. Opening it suppresses local gameplay
actions and sends neutral intent while the server continues; the player remains vulnerable. Leaving
an active match requires confirmation and applies the existing server-owned forfeit policy.
Scoreboard behavior may remain a non-blocking hold overlay if playtesting shows that it should not
suppress gameplay input.

```text
Title ── Play ──► ServerSelect ── Join ──► Connecting
  │                    ▲                       │
  │                    │ disconnect            │ handshake/catalog ready
  │                    │                       ▼
  ├── Practice         └────────────────── GameSelect ◄─────────────┐
  │                                            │                    │
  │                                      queue exact game type      │
  │                                            ▼                    │
  │                                          Queue ── cancel ───────┘
  │                                            │
  │                                      tickets reserved
  │                                            ▼
  │                                      MatchLoading
  │                                  sync + formation check-in
  │                                            │
  │                                  authoritative countdown
  │                                            ▼
  │                                           Match
  │                                            │
  │                                         completed
  │                                            ▼
  │                                          Results
  │                                   queue again / change game
  └─────────────────────────────────────────────────────────────────

Overlays: BuildEditor, Settings, Credits, InMatchMenu, Scoreboard,
          Confirmation, Error
```

### Screen inventory

| Flow/overlay | Purpose | Primary content | Backing state today |
|---|---|---|---|
| **Title** | Product hub | Play, Practice, Settings, Credits, Quit; logo/version | Missing |
| **ServerSelect** | Choose a known server | Direct address, explicit favorites, recent servers, Back | `--server` only |
| **Connecting** | Establish a lobby session | DNS/transport, handshake, compatibility, game-type catalog; Cancel | Partial `ClientJoinPhase`; currently also assumes match readiness |
| **GameSelect** | Choose an advertised game type | Mode, map pool, topology, rules summary, pool count; edit build | Missing advertisement/UI |
| **BuildEditor** | Create the next queue build | Presets, bounded fields, budget, stats, confirm/cancel | Debug-only overlay; validation exists |
| **Queue** | Wait for exact formation | Game type, `queued/needed`, accepted build, Cancel | Missing |
| **MatchLoading** | Prepare one reserved match | Selected map/mode, map/assets/terrain sync, participant check-in, timeout | Existing readiness pieces, wrong lifecycle location |
| **Match** | Countdown and active play | Product HUD, crosshair/range, objective, cues | Debug-heavy presentation |
| **InMatchMenu** | Non-pausing overlay | Resume, Settings, Scoreboard, Leave Match, debug toggle | Debug pause overlay |
| **Scoreboard** | Match roster and score detail | Teams, names, builds where policy permits, score/objective | Text overlay |
| **Results** | Completed-match decision | Winner, final result, team summary, Queue Again, Change Game, Disconnect | Debug phase overlay/restart quorum |
| **Settings** | Local configuration | Input, accessibility, audio, display/window | M11 data, UI missing |
| **Credits** | Attribution and licenses | Required asset attribution and project credits | Missing |
| **Error** | Recoverable failure overlay | Plain-language reason and context-valid Retry/Back action | Structured categories partly planned in M11 |

### Transition and membership rules

- `Connecting` completes after transport, handshake/fingerprint validation, session identity, and a
  bounded game-type advertisement. It does not wait for a map or terrain instance.
- GameSelect and BuildEditor are lobby presentation. The editor changes a local draft; successful
  queue admission validates and freezes its server-owned snapshot on the ticket.
- Cancel Queue removes the ticket and returns to GameSelect without disconnecting.
- Exact formation reserves tickets and allocates a match worker; it does not begin gameplay
  immediately.
- MatchLoading closes the lobby Lightyear session, establishes a fresh worker session through the
  same public endpoint, synchronizes the selected map/assets/terrain, and performs a bounded
  check-in. Once every retained participant is ready, the worker starts the only gameplay
  countdown.
- If formation loading fails or a participant times out, the server disposes the incomplete match
  instance and returns valid remaining tickets to the front of their pool under a specified bounded
  retry policy.
- Leave Match confirms and forfeits, closes the match-worker connection, and establishes a fresh
  lobby connection through the same logical server endpoint before returning to GameSelect.
- Results has independent Queue Again, Change Game, and Disconnect actions. There is no player-facing
  restart quorum or fixed-roster rematch.
- Disconnect from Server is the only ordinary transition back to ServerSelect from an active lobby.
- Retry after connection loss starts a fresh lobby session. V2 does not imply match resumption or
  join-in-progress.
- All membership commands carry request/ticket identity and are idempotent under duplicates,
  cancellation/formation races, and late responses.

## Build selection contract

The current recipe catalog, resolver, preset profiles, and 12-point validation remain the gameplay
authority. The lifecycle changes:

1. The BuildEditor owns a local editable draft and previews locally resolvable values.
2. Queue Join carries the complete bounded recipe or stable preset identity plus required catalog
   revision.
3. The server resolves and validates the build atomically with ticket admission.
4. The accepted immutable public build snapshot belongs to the ticket and transfers into the formed
   match's selected-build/loadout lifecycle.
5. The player cannot counter-pick while queued. Editing requires Cancel Queue and re-admission, or a
   future explicitly specified atomic ticket update; the initial v2 implementation should prefer
   cancel and requeue.
6. Queue UI shows the player's own accepted build. Opponent builds are not exposed before formation;
   the match scoreboard exposes only the bounded public configuration needed for readable play.
7. A schema-versioned local last-used build improves repeat sessions. Named account-owned arsenals,
   acquisition, entitlements, and cloud persistence remain deferred under `FUT-ARSENAL`.

## Session identity and local persistence

- Stable `PlayerId`/network identity remains authoritative; display name is presentation metadata,
  never identity or authorization.
- V2 display names are session-scoped and untrusted. The server enforces UTF-8 byte and grapheme
  bounds, normalization, control-character rejection, and deterministic duplicate suffixes, then
  replicates the accepted display form.
- A generated usable default means controller play never blocks on text entry.
- Local files for settings, favorites, recents, display name, and last-used build have explicit
  schema versions, platform-appropriate locations, bounded input, atomic replacement, and safe
  fallback after missing or malformed data.
- Favorites are player-authored; recents are bounded and automatic. Neither belongs in the server
  feature graph.

## Session and protocol work

| Item | Status | Agreed direction |
|---|---|---|
| Non-fatal windowed lifecycle | Debug-only → change | Transition to flow/error UI; retain headless exit contracts |
| Lobby session identity/name | Missing | Server-sanitized session display name over stable numeric identity |
| Game-type advertisement | Missing | Bounded stable IDs/revisions, modes, map pool, exact topology, rules summary, and queue count |
| Queue commands and snapshots | Missing | Reliable in-band requests/acks with ticket identity, revisioned aggregate pool state, and explicit rejection reasons |
| Match reservation/loading | Missing | Match allocation, targeted sync, check-in deadline, dissolution/requeue policy |
| Leave/cancel/disconnect | Missing | Separate idempotent intents with distinct membership effects |
| Address and local server lists | Missing | Validated hostname/address, explicit favorites, bounded recents |
| Match recovery | Partial | Terrain/map recovery applies after reservation; no v2 session resumption |
| Public registry | Deferred | Coordinate later with internet reachability and public-server policy |

Lobby, build, and queue actions stay on dedicated reliable in-band Lightyear messages/channels on
the lobby connection. Match loading and gameplay actions use the separately authenticated match
worker connection. The routing envelope and IPC framing live below Lightyear and never become a
second gameplay protocol. REST/HTTP becomes a new architecture decision only when a concrete
external service—such as accounts, inventory, registry, or cross-server matchmaking—requires it.

## Multi-process architecture research contract

The topology is decided, but its reusable implementation begins with the v2 transport-foundation
milestone. That milestone must answer and prove:

| Concern | Required result |
|---|---|
| Process ownership | The supervisor owns ingress, routing capabilities/tables, host capacity, worker handles, and shutdown; a long-lived lobby worker owns sessions/queues/reservations and delivers supervisor-minted capabilities; each match worker owns exactly one authoritative match world |
| Routed transport | One public UDP socket routes opaque Lightyear datagrams through bounded framed IPC while preserving packet and peer identity |
| Connection handoff | Lobby and match use distinct Lightyear sessions at the same public endpoint; expired, replayed, malformed, and misrouted capabilities fail safely |
| IPC portability | One validated frame codec supports message-oriented backends directly and explicit length-prefixed byte streams, including the selected macOS/Linux backend and a documented Windows named-pipe or portable fallback path |
| Backpressure | Per-route and per-worker queues, packet-size limits, drop behavior, and telemetry are bounded and tested under a stalled worker |
| Worker lifecycle | Spawn, manifest validation, readiness, heartbeat, result, graceful stop, forced termination policy, crash detection, and route cleanup are explicit |
| Gameplay reuse | The production match worker composes the existing server-authoritative plugins; the foundation is not a mock gameplay server or disposable example |
| Security | Routing capabilities authorize routing, not player identity or a replacement for inner Netcode authentication; they are unguessable, scoped, expiring, revocable, and rate-limited before allocating worker or route state |
| Admission budgets | Host-wide ceilings bound workers, fighters, memory, CPU/fixed-tick work, IPC queues, and bandwidth based on measurement |
| Verification | Two simultaneous worker matches, cross-route isolation, reconnect/handoff, crash cleanup, impaired networking, and repeated lifecycle soaks pass |

The first UX vertical slice may use one active worker while this gate is researched, but the v2
queue/concurrency feature is not complete until these invariants pass. The foundation milestone
must leave production modules, tests, and observability that later lobby and formation milestones
extend; it must not leave a throwaway prototype beside the real server path.

## In-match presentation scope

| Item | Status | V2 result |
|---|---|---|
| Combat HUD | Debug-only → replace | Health, ammo/cooldown, weapon/build identity, ultimate charge, score/objective, and bounded alerts using icons/bars/pips |
| Crosshair/range/landing | Have | Restyle and validate for controller readability |
| Objective presentation | Have | Preserve authoritative facts; restyle Wipeout and Hot Zone presentation |
| Scoreboard | Text → replace | Readable team panel on View/in-menu; no queue-wide identity disclosure |
| In-match menu | Debug-only → replace | Non-pausing overlay with neutral local intent and confirmed leave |
| Results | Debug-only → replace | Final result plus Queue Again, Change Game, and Disconnect |
| Debug overlay | M11 | Remains separately toggleable and never becomes the product HUD |
| Combat feedback | Have/partial | Preserve hit/defeat feedback; add only v2 readability polish, not the full production VFX pipeline |

## Settings, accessibility, and text input

V2 settings cover:

- M11 action remapping, deadzones, aim threshold, sensitivity, Y inversion, conflict detection,
  reset-to-default, controller disconnect, and active-device glyph changes;
- master/SFX/music-ready volume categories, mute-on-focus-loss policy, and safe defaults even if v2
  has no full music catalog;
- window mode, resolution, vsync/frame limit, cursor capture, UI scale, and safe layout margins;
- non-color-only team identification and a colorblind-friendly palette option;
- reduced screen shake, reduced flashes, and bounded presentation intensity;
- focus repeat/debounce, focus restoration after modal/list changes, mouse hover/click, and clear
  focus indication;
- keyboard entry and paste for addresses and optional names. Controller users can complete every
  ordinary flow by using generated names and saved servers without an on-screen keyboard.

Credits are a top-level Title destination and may also be linked from Settings/About, but remain one
owned screen. Required attribution derives from the existing asset manifest.

## Error and retry policy

The error presentation maps structured categories to honest actions:

- invalid address/name/local file → correct locally or restore defaults;
- DNS/transport timeout → retry the same server or return to ServerSelect;
- protocol/content mismatch → explain incompatibility; do not loop Retry automatically;
- server full/game type unavailable/config revision changed → refresh GameSelect where the lobby
  session remains valid;
- queue rejection/build rejection → remain in GameSelect/BuildEditor with the precise correctable
  reason;
- formation failure/check-in timeout → return valid tickets according to the documented retry
  policy and explain the outcome;
- match disconnect → acknowledge that the match cannot resume, then retry a fresh lobby session or
  return to ServerSelect;
- server shutdown → return to ServerSelect with a non-crash message.

Errors may be overlays, but a lost connection changes the underlying flow and clears stale queue,
match, map, terrain, focus, and presentation state exactly once.

## Verification requirements

- **Pure/UI tests:** flow/overlay transitions, return destinations, focus graph and restoration,
  controller/KBM parity, build budget display, local-file migration/fallback, and error mapping.
- **Schedule/ECS tests:** presentation cannot mutate authoritative simulation; match-scoped queries,
  mode routing, deferred boundaries, and exact cleanup remain explicit.
- **Protocol tests:** bounded advertisements, names, recipes, ticket IDs/revisions, duplicates, stale
  commands, malformed input, rate limits, and direction/target registration.
- **Network tests:** connect → catalog → queue → reserve → load/check-in → countdown → match →
  results → requeue/change-game; rejection → correction → rejoin; queue overflow; cancellation at
  formation; disconnect in every phase; fresh-session retry; and headless exit equivalence.
- **Concurrency tests:** simultaneous Wipeout/Hot Zone worker processes, route/message/recovery
  isolation, host admission refusal, worker teardown/crash, and repeated formation/completion soak.
- **Performance tests:** measured server fixed-tick, entity, terrain, memory, and bandwidth ceilings;
  client FPS/frame-time and UI rebuild targets at supported resolutions.
- **Visual/accessibility checks:** minimum resolution, window resizing, ultrawide behavior, UI scale,
  safe margins, non-color team reading, reduced-effects mode, controller disconnect, and input-glyph
  switching.
- **Usability checks:** first-run Practice reaches controllable play within one minute; a populated
  favorite requires only a handful of inputs; queue state never promises an unsupported wait time.

## Remaining milestone R&D questions

These are technical research tasks rather than unsettled product behavior:

1. Which framed IPC primitive and wake-up strategy best satisfies packet boundaries, bounded
   backpressure, clean shutdown, and the supported desktop platforms?
2. What minimal Lightyear transport plugin maps routed IPC peers to `Link` entities without
   modifying Lightyear connection, replication, or gameplay layers?
3. What role-specific manifest, restart, reconciliation, and shutdown policy does the long-lived
   lobby worker require beyond the process/IPC machinery shared with match workers?
4. What measured host ceilings allow concurrent 2v2/3v3 worker processes with independent terrain
   while preserving fixed-tick, memory, IPC, and bandwidth budgets?
5. What exact deterministic team-assignment and map-rotation algorithms are simplest and testable?
6. What bounded check-in timeout and ticket-return policy behaves best under local and impaired
   network profiles?
7. Which platform path and atomic persistence mechanism should store the versioned client settings,
   favorites, recents, name, and last-used build?
8. How should the product launcher invoke the same supervisor/worker topology for local practice
   across startup, readiness, failure, and shutdown without adding a second gameplay path?

These questions belong in the relevant v2 milestone research sections. They do not reopen the
agreed authority, membership, formation, rematch, or global-matchmaking boundaries above.
