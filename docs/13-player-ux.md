# Player UX and server-local matchmaking design

## Purpose and scope

This document is the accepted V2 product-direction record. V2 completed the product client shell,
direct-connect flow, server-local queues, automatic match formation, isolated authoritative match
workers, HUD/settings/accessibility baseline, and server-hosted practice on 2026-08-20. Future-tense
language below records the design as reviewed; the V2 roadmap and milestones contain exact
implementation evidence. V3 subsequently replaced gameplay-world presentation without changing
this player-flow boundary.

V5 supersedes the V2 navigation shell without rewriting this historical authority record. Normal
startup now auto-connects and enters the Player Dashboard; Server Select is the recovery/manual
connection surface. Game Type Select and Build Editor are Dashboard children with Confirm/Back.
Dashboard Play and Practice are the only ordinary admission initiators. Queue/loading cancellation,
confirmed leave, and no-result lobby return converge on Dashboard, while Results exposes only exact
fresh-catalog replay and Dashboard. The current map is maintained in
[PewPew Blitz screen-flow audit](./screen-flow-map.md); any Title-first or Game-Select-hub language
below describes the completed V2 baseline, not the V5 product flow.

V5 M03 hardens that current shell across the validated desktop window/UI-scale range. The Dashboard
uses the accepted Wide hierarchy when effective UI space is at least `1000x640` and a vertically
scrollable Compact hierarchy below either threshold. It keeps the actual gameplay model and preview
target alive while resizing, follows keyboard/gamepad focus through the compact scroll, skips
disabled targets, exposes factual accessible labels, and freezes the procedural field under Reduced
Motion or Reduced Effects. These rules are presentation-only and do not alter session, admission,
or gameplay authority.

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

## V2 feature boundary (completed)

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
6. Exact 1v1, 2v2, and 3v3 Wipeout and Hot Zone game types, subject to mode/map validation and each
   milestone's R&D.
7. A redesigned combat HUD, scoreboard, in-match menu, results flow, and non-fatal session
   lifecycle.
8. An authoritative bot-practice path where a first-time player can select any compatible game type
   and test controls and builds without a populated server queue.
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
is a thin supervisor/router plus a long-lived lobby worker and isolated match worker processes.
Practice runs on the connected server through that same topology; the windowed client never launches
a server process, owns gameplay authority, or bypasses routed validation and server-owned
simulation. The proposed topology and its remaining R&D gates are recorded in
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

```ron
(
  id: "first-blood",
  revision: 2,
  name: "First Blood",
  mode: "wipeout",
  maps: ["ashen-court"],
  teams: 2,
  players_per_team: 1,
  kills_to_win: 1,
  match_duration_seconds: 180,
  countdown_seconds: 3,
  respawn_seconds: 3,
)
```

Every game repeats its own flat timing values; there is no shared defaults block or operator-facing
rules profile. Wipeout declares `kills_to_win`; Hot Zone declares `capture_seconds`. Startup
validation proves the objective/mode pairing, positive bounded timings, map anchors and spawn
capacity, formation size, content identities/revisions, and process ceilings before advertisement.
The resolved values are carried into the isolated match worker and installed as authoritative ECS
rule resources. The server selects among multiple allowed maps by deterministic rotation; clients
do not vote or author rules in v2. Runtime hot-editing is deferred.

Practice reuses the connected server's validated game-type advertisements without entering a
multiplayer pool. After the player selects a compatible type, the lobby requests ordinary
supervisor capacity for a match worker containing that player and bots in the remaining roster
positions. A full server rejects the request immediately rather than creating a practice wait list.
M08 bots are inert named participants with no AI; compatibility validates only that ordinary mode
rules and lifecycle accept the complete roster. An incompatible type is marked unavailable rather
than falling back to different rules. Bot behavior is deferred to a later version.

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
- Multiplayer pools contain human tickets only and retain exact-human formation in M08. Practice
  does not join, wait in, or fill those pools.

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

## Pre-V2 prototype state (historical)

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
   offered and starts immediately when the selected server is reachable and has worker capacity; a
   reachable populated favorite is a short path to PvP. The UI never invents a short queue estimate
   when the population cannot support one.
2. **Controller-first navigation, keyboard/mouse first-class.** Focus navigation, confirm, cancel,
   and back are consistent. Direct address and optional name editing may use keyboard/paste;
   controller users can accept a generated name and navigate saved servers without text entry.
3. **One place for every choice.** Game type and build are selected before queue admission. The
   build is locked while queued; changing it means cancelling or completing an acknowledged ticket
   update before formation.
4. **Queue state is honest and private.** Before reservation, show game type,
   `N waiting · M players per match` and the player's accepted build, aging stale population to an
   updating state rather than presenting it as current. Show reservation/loading progress only after
   formation owns that state. Do not publish every waiting player's name or build.
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
| **GameSelect** | Choose an advertised game type | Mode, map pool, topology, rules summary; M04 adds honest pool state and build editing | Missing advertisement/UI |
| **BuildEditor** | Create the next queue build | Presets, bounded fields, budget, stats, confirm/cancel | Debug-only overlay; validation exists |
| **Queue** | Wait for exact formation | Game type, fresh `N waiting · M players per match`, accepted build, Cancel; reservation/loading progress begins after M05 formation | Missing |
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
| Game-type advertisement | Missing | Bounded stable IDs/revisions, modes, map pool, exact topology, and rules summary; revisioned queue counts begin with real M04 pools |
| Queue commands and snapshots | Missing | One ordered-reliable in-band client envelope preserves request/ack order with ticket identity; only sessions authenticated at frame start may issue queue commands; equivalent re-Join preserves the ticket and original admission revision; pending identical recovery does not spend semantic-command tokens or enqueue another copy at the bounded ten-second Retry cadence, while repeated early duplicates disconnect; the first over-rate new command fails softly before continued abuse disconnects; complete revisioned aggregate snapshots use bounded sequenced delivery plus refresh, byte-equivalent current-revision refreshes renew freshness while older snapshots do not, and explicit actionable rejection reasons, outcome-to-snapshot freshness barriers, and privacy-safe bounded diagnostics remain required |
| Match reservation/loading | Missing | Match allocation, targeted sync, check-in deadline, dissolution/requeue policy |
| Leave/cancel/disconnect | Missing | Separate idempotent intents with distinct membership effects |
| Address and local server lists | Missing | Validated hostname/address, explicit favorites, bounded recents |
| Match recovery | Partial | Terrain/map recovery applies after reservation; no v2 session resumption |
| Public registry | Deferred | Coordinate later with internet reachability and public-server policy |

Lobby, build, and queue commands/outcomes stay on dedicated reliable in-band Lightyear messages on
the lobby connection. When acknowledgement and command order affects authority, one typed client
envelope preserves their channel order rather than reconstructing it across typed receivers.
Replaceable complete aggregate snapshots may use a bounded sequenced channel
with periodic refresh because membership transitions never depend on those snapshots. Match loading
and gameplay actions use the separately authenticated match-worker connection. The routing envelope
and IPC framing live below Lightyear and never become a second gameplay protocol. REST/HTTP becomes
a new architecture decision only when a concrete external service—such as accounts, inventory,
registry, or cross-server matchmaking—requires it.

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
| Combat HUD | Debug-only → replace | A minimal gameplay-only surface: health, ammo/reload, item/ultimate readiness, time/phase, and bounded status alerts using labels/bars/pips. Do not keep player IDs, connection/input state, raw ticks, controls help, or other development facts here |
| Crosshair/range/landing | Have | Restyle and validate for controller readability |
| Objective presentation | Have | Preserve authoritative facts; restyle Wipeout and Hot Zone presentation |
| Scoreboard | Text → replace | Readable team panel on View/in-menu; no queue-wide identity disclosure |
| In-match menu | Debug-only → replace | Non-pausing overlay with neutral local intent and confirmed leave |
| Results | Debug-only → replace | Final result plus Queue Again, Change Game, and Disconnect |
| Mode score/objective | Debug-only → replace | Reserve the top-right HUD slot for the active mode's compact score/objective model; Wipeout and Hot Zone compose only their own replicated facts into the shared slot |
| Debug overlay | M11 | Remains separately toggleable as diagnostics mode and never becomes the product HUD |
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
- server full/game type temporarily unavailable → retain the valid lobby session and offer the
  context-valid retry/back action;
- catalog or game-configuration disagreement that is impossible under the accepted immutable
  advertisement → close the incompatible lobby session and offer a fresh connection or ServerSelect;
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
- **Protocol tests:** bounded advertisements, names, recipes, ticket IDs/revisions, frame-start
  authentication eligibility, duplicates and bounded identical-retry cadence, stale commands,
  malformed input, semantic rate limits, and direction/target registration.
- **Network tests:** connect → catalog → queue → reserve → load/check-in → countdown → match →
  results → requeue/change-game; rejection → correction → rejoin; queue overflow; cancellation at
  formation; disconnect in every phase; fresh-session retry; bounded reliable queue outcomes;
  consecutive aggregate-snapshot loss/aging/recovery, byte-equivalent current-revision freshness
  renewal, older-snapshot rejection; and headless exit equivalence.
- **Concurrency tests:** simultaneous Wipeout/Hot Zone worker processes, route/message/recovery
  isolation, host admission refusal, worker teardown/crash, and repeated formation/completion soak.
- **Performance tests:** measured server fixed-tick, entity, terrain, memory, and bandwidth ceilings;
  client FPS/frame-time and UI rebuild targets at supported resolutions.
- **Visual/accessibility checks:** minimum resolution, window resizing, ultrawide behavior, UI scale,
  safe margins, non-color team reading, reduced-effects mode, controller disconnect, and input-glyph
  switching.
- **Usability checks:** first-run Practice reaches controllable play within one minute against a
  reachable server with capacity; a populated favorite requires only a handful of inputs; queue
  state never promises an unsupported wait time.

## Remaining milestone R&D questions

These are technical research tasks rather than unsettled product behavior:

1. Which framed IPC primitive and wake-up strategy best satisfies packet boundaries, bounded
   backpressure, clean shutdown, and the supported desktop platforms?
2. What minimal Lightyear transport plugin maps routed IPC peers to `Link` entities without
   modifying Lightyear connection, replication, or gameplay layers?
3. What role-specific manifest, restart, reconciliation, and shutdown policy does the long-lived
   lobby worker require beyond the process/IPC machinery shared with match workers?
4. What measured host ceilings allow concurrent 1v1/2v2/3v3 worker processes with independent
   terrain while preserving fixed-tick, memory, IPC, and bandwidth budgets?
5. What exact deterministic team-assignment and map-rotation algorithms are simplest and testable?
6. What bounded check-in timeout and ticket-return policy behaves best under local and impaired
   network profiles?
7. Which platform path and atomic persistence mechanism should store the versioned client settings,
   favorites, recents, name, and last-used build?
These questions belong in the relevant v2 milestone research sections. They do not reopen the
agreed authority, membership, formation, rematch, or global-matchmaking boundaries above.

The practice-hosting question is resolved for specification review by
[`v2/milestone-08.md`](./implementation/v2/milestone-08.md): Practice runs on the connected server,
bypasses multiplayer queues, and uses ordinary supervisor capacity to allocate one authoritative
match worker for the player and bots. The client launches no helper processes.
