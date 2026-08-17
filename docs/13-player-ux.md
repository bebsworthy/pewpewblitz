# Player UX and presentation design

## Purpose and scope

This document analyzes the current prototype presentation, the experience the design docs imagine,
and the screens, states, and transitions that bridge the gap. It is a design and gap analysis, not
a milestone specification: it identifies what must be built and what is underspecified, and it
respects the v1 non-goals (no production matchmaking services, accounts, parties, or backend
services). Basic server-local pool matchmaking (queue a game type, start when enough players are
queued) is part of the target UX; ELO/skill matching, cross-server queues, and party services are
not.

Status triage used below:

- **Have** — exists in the prototype, usable or reusable.
- **Debug-only** — exists but is text/debug presentation that must be replaced by product UI.
- **Missing** — required for the described experience; not built.
- **Design gap** — the docs never settled the design; the milestone must decide before building.

## Hosting topology and matchmaking (decision)

The client never creates a server. Servers are hosted independently (dedicated `brawler-server`
processes, local for development, remote later) and clients join them:

- **Join by address is always available:** `IP:port` or `hostname:port` (with an optional default
  port when omitted), validated before any network attempt.
- **Favorites are a basic TOML list** (`server_name`, `addr`), auto-saved when the player joins or
  edits, and hand-editable since the file is plain text.
- **Discovery is a later service, not matchmaking.** A future registration endpoint receives
  heartbeats from dedicated servers (name, addr, available game types, players, version,
  protocol/content fingerprints) and serves a list the client can browse. A registry lists
  servers; it does not solve client-to-server reachability (NAT, relays — see `GAP-NET-INTERNET`).
  Until it exists, the address entry and favorites list are the complete join experience.
- **The server operator defines the games available on that server.** Server configuration lists
  one or more game types, each binding a mode to the allowed maps and settings for that mode:

  ```toml
  [[game_types]]
  name = "Wipeout 2v2"
  mode = "wipeout"
  maps = ["arena"]
  rules = { target_score = 10, max_players_per_team = 2 }

  [[game_types]]
  name = "Hot Zone 2v2"
  mode = "hot-zone"
  maps = ["hot_zone_arena"]
  ```

  Clients never author rules, maps, or settings; they choose among the server's offered game
  types. The "HostSetup / create a game" screen is removed from the player UX.
- **Pool matchmaking, server-local and skill-free.** A player joins a game type (optionally
  filtered by game mode) and enters that type's pool of waiting players, with their selected
  build. When the pool has enough players for one game of that type, the server forms a match and
  starts it after a short formation countdown. There is no ELO, rank, or skill matching — that is
  explicitly pointless until the player base exists. Overflow players stay queued for the next
  game.
- **Concurrent matches per process are the intended design.** A server hosts many game types and
  many pools, and formed matches run in parallel on the same process. This re-instances the
  existing single-match machinery rather than writing a parallel model: per-match rules and
  capacity (today `MatchLifecycleRules`/`ResolvedMatchCapacity` are process resources), per-match
  map and terrain instances (the M10 chunk grid is global), per-match fixed-tick budgets (engine
  ceilings currently assume one match), admission by match id, and coarse interest management so
  clients only receive their own match. This design depends on `GAP-NET-ROOMS` (Lightyear room
  interest management); ordering and gap analysis are separate, later work.

## Current state (what the prototype does today)

The windowed client connects immediately on launch and never shows a menu:

- **Auto-connect on boot.** `ClientNetworkConfig::new(1)` connects to `127.0.0.1:5000`; the server
  address, client id, build preset, and mode come from command-line flags (`--server`,
  `--client-id`, `--build-preset`, server `--mode wipeout|hot-zone`). No launch screen, no
  server selection, no game creation.
- **Join lifecycle is a sequence, not a flow.** Connecting → handshake (`ClientHello`) →
  `ClientJoinPhase::Active` or `Rejected`/`Disconnected`. Rejection and disconnect write
  `AppExit::error()`: the client quits instead of returning to a menu.
- **Build selection is a debug overlay.** During the waiting phase, arrow keys cycle four presets
  plus one custom recipe (pulse power/reach/magazine, ultimate, two passives) over a fighter. All
  text, driven by `update_build_selection_overlay`. The server already validates preset and custom
  `BuildSelectionRequest`s against a 12-point budget, so the authority side is product-ready.
- **The waiting phase is the lobby.** When every participant is ready, the server counts down and
  starts; on completion, restart unlocks and a restart quorum repeats the match. There is no
  "leave", no host control, and no mode/map choice after server launch. This machinery is the
  foundation the pool model replaces or reuses (see underspecified items 2–4).
- **The in-match HUD is a text console.** Readiness text ("CONNECTING", "SYNCING TERRAIN", "READY"),
  a match line ("WIPEOUT | P1 T1 | 3-1 / 10 | 2:31"), a roster dump ("P1 T2 W3 alive"), a countdown
  number, and a phase overlay ("GET READY", "TEAM 2 WINS", restart prompts). Correct and complete,
  designed for debugging, not playing.
- **No player identity.** Players are "Brawler client {id}". No display names anywhere.
- **No settings or credits.** Input remapping is M11 scope but has no UI yet; CC0 attribution has
  no credits screen.
- **Headless automation exists and must survive.** `--headless`, demos, and network tests drive the
  same authoritative path without UI.

## What the docs imagine

- **Controller-first menus and combat, KBM fully supported.** "Make primary fire, active item, and
  ultimate states readable without requiring a mouse cursor"; "Do not make precise cursor placement
  mandatory for objectives or menu navigation" ([Gameplay MVP](../05-gameplay-mvp.md#controller-usability-requirements)).
  Menus must be navigable by focus ring with A confirm / B cancel.
- **Buildcraft is the differentiation.** The arsenal of player-authored brawlers (weapon recipe +
  body + ultimate + items) is the long-lived player-facing content ([Product direction](../00-product-direction.md#differentiation),
  `FUT-ARSENAL`). The MVP exposes four presets plus one bounded custom recipe; a full editor,
  persistence, and acquisition are post-v1.
- **Short matches, short sessions.** "A match should expose a build's strengths and weaknesses
  quickly" — getting from launch to fighting should be a handful of inputs.
- **Readable competition.** Players must understand why they were damaged, slowed, defeated, or
  denied an objective — including team/mode state in the queue and match.
- **Mode and map are server-owned.** Players may pick modes and maps for a match they host; they
  never author rules ([Product direction](../00-product-direction.md#creator-direction)).
- **MVP networking is a local dedicated server plus clients.** Two local clients complete an
  authoritative match; in-process loopback is a dev convenience ([roadmap network policy](../implementation/v1/roadmap.md#network-and-protocol-policy)).
  Production matchmaking services are a non-goal, which the pool model respects: queues are
  server-local and skill-free, and address-based joining always works.
- **M11 delivers input remap/calibration data and a debug overlay** but no full menu shell
  (`GAP-UI-SETTINGS`, `GAP-AUDIO-SETTINGS`, `GAP-UI-WINDOW`, `GAP-LEGAL-CREDITS` in the
  [backlog](../backlog.md)). A bot practice mode is explicitly permitted by the network architecture
  doc (`GAP-MODE-TRAINING`).

## Design principles for the ideal experience

1. **Boot to a menu, fight within a minute.** The launch screen is the hub; every path to a match
   is short.
2. **Controller-first navigation, KBM as first-class.** A focus ring moves with D-pad/left stick
   and Tab/arrows; mouse click and hover work everywhere; confirm/cancel always map to A/B and
   Space/Enter/Escape.
3. **One place for every choice.** Game type, build, and mode filter are decided before queuing;
   the match itself only plays.
4. **The queue is honest about state.** Game types, pool sizes, expected formation (players
   queued vs. needed), and build are visible while waiting; the match preview is visible during
   the formation countdown.
5. **Fail soft.** Rejection, timeout, mismatch, and disconnect return to a menu with a clear
   message. A prototype may quit; a game never does.
6. **Nothing authoritative moves to the client.** Every screen is presentation over replicated or
   local state; menus send intent (`SetReady`, build selection, leave) through the existing
   validated channels.
7. **Automation survives.** Headless clients, demos, and network tests bypass the UI through the
   same join path, never through a fake UI.

## Ideal experience: screens, states, and transitions

Client screens are a Bevy state machine (`States`) named `ClientScreen`. One screen is active at a
time; the match overlays (menu, settings, scoreboard) are sub-states of `Match`.

### There is no pause in multiplayer

A multiplayer match cannot pause: the authoritative server and every other player keep running.
The menu button opens an in-match **Menu overlay** while the match continues unchanged (the
current prototype already behaves this way — the pause toggle is local and the server keeps
ticking). The overlay exists for settings, scoreboard, and leaving; it never stops the match, and
no player is given a free clock-stall. "Pause" is legacy naming; the concept is *menu overlay*.

```text
Title ───────────────► ServerSelect ◄──────────────┐
  │                       │                        │
  │                       │ Join                   │ error/leave
  │                       ▼                        │
  │                  Connecting ───────────────────┤
  │                       │                        │
  │                       ▼                        │
  │                  GameSelect ─────► BuildEditor ─┤
  │                       │                        │
  │            Join game type (mode filter)        │
  │                       ▼                        │
  │                    Queue ──────────────────────┤
  │                       │                        │
  │          pool fills (server-owned)             │
  │                       ▼                        │
  │               FormationCountdown ─────────────►┤
  │                       │                        │
  │                       ▼                        │
  │                    Match ◄──► Menu              │
  │                       │                        │
  │                       ▼                        │
  │                   Results ◄─ (completed)       │
  │                       │                        │
  │            Queue again / leave ───────────────►┤
  │                       │                        │
  │                       ▼                        │
  │                    Queue ◄─────────────────────┘
  │
  ▼
Settings ◄───────────────────── any screen (sub-state)
Credits
```

Note: `FormationCountdown` is followed by the existing `MatchState` waiting/countdown sequence
where eligible — or by a short server-side formation check-in — and after `Results` the player
re-queues rather than using restart quorum. Pools form matches concurrently; each formed match
runs as its own match instance on the same server process (see the concurrent-matches design
below).

### Screen inventory

| # | Screen | Purpose | Primary content | Backing state today |
|---|---|---|---|---|
| 1 | **Title** | Hub: Play (join), Settings, Credits, Quit | Logo, version, focus menu | none — client boots straight to connect |
| 2 | **ServerSelect** | Join a server by address or from favorites | Direct address entry (`IP:port`, `hostname(:port)`), favorites list, (future: registry list), Back | none; `--server` flag only |
| 3 | **Connecting** | Transient: connect, handshake, game-type sync, map/terrain sync, assets | Progress line from the existing readiness ladder, Cancel | `ClientJoinPhase`, `ClientMapReadiness`, `ClientTerrainReadiness`, `ClientAssetReadiness` |
| 4 | **GameSelect** | Choose a game type on this server | Game-type list (mode + maps + settings + pool size), optional mode filter, join; Build edit entry | none — server advertises game types (new message) |
| 5 | **BuildEditor** | Choose preset or edit the bounded custom build before queuing | Preset cards, custom fields, budget bar (12 pts), stat/profile preview, Confirm | `BuildCatalogResource`, `resolve_build_recipe`, `BuildSelectionState` |
| 6 | **Queue** | Waiting in a pool | Game type, players queued vs. needed, current build, Cancel; player names appear as they queue | none — new pool replication/messages |
| 7 | **Match** | Countdown + active play | Minimal HUD: health/ammo, weapon, ultimate charge, score/objective, crosshair + range, landing indicator | combat HUD + match state |
| 8 | **Menu** | In-match overlay; the match never pauses | Resume, Settings, Scoreboard, Leave Match, Debug overlay toggle | `ClientInputContext::Paused` (legacy name) |
| 9 | **Results** | Match completed | Winner, final score/objective, team summary, Queue again (same game type), Leave | `MatchPhase::Completed` |
| 10 | **Settings** | Global settings | Input remap/calibration (M11), audio, display/window, credits | M11 `ClientInputSettings` |
| 11 | **Error** | Modal: rejection, mismatch, timeout, disconnect | Reason in plain words, Back to ServerSelect, Retry | `JoinRejection`, `ClientJoinPhase::Rejected/Disconnected` |

### Key transitions and their rules

- **Title → ServerSelect** via Play. **ServerSelect → Connecting** on Join: the address is
  validated (`SocketAddr`, or hostname resolved with a default port) before any network attempt.
  Favorites are a plain TOML list (`name`, `addr`) auto-saved on join/edit and hand-editable.
- **Connecting → GameSelect** on `ClientJoinPhase::Active` + game-type advertisement + asset
  readiness. **Connecting → Error** on rejection, timeout, or mismatch.
- **GameSelect ↔ BuildEditor** happens before queuing. The build is still sent and validated
  exactly like today (`BuildSelectionRequest`); the editor is its presentation. A change of build
  while queued re-sends the selection; server re-validates.
- **GameSelect → Queue** on "Join": the client requests membership in that game type's pool
  (server-validated against capacity and legality). **Queue → GameSelect** on Cancel (leave pool).
- **Queue → Match** is server-owned: when the pool reaches the game type's formation size, the
  server forms the match, runs a short formation countdown (straggler grace), assigns teams, and
  starts. The current all-ready start and restart-quorum machinery is superseded by pool formation;
  whether restart quorum survives as an in-group rematch is a design decision below.
- **Match → Menu** toggles locally; the authoritative match never pauses and continues (unchanged).
  **Match → Results** on `Completed`.
- **Results → Queue** re-queues the same game type (same build). **Results → ServerSelect** leaves.
- **Leave Match / Leave Queue** sends an intentional departure and returns to ServerSelect. The
  server keeps its existing departure/forfeit semantics; the client simply disconnects cleanly
  instead of quitting.
- **Error → ServerSelect** always. No prototype-quit behavior remains in the windowed client;
  headless automation keeps its exit codes for CI.
- **Future registry browse** (a later service): the server list tab populates from the registration
  endpoint's heartbeat data (including the offered game types); joining still uses the same
  address-based join path.

## What must be built to get there

### A. Client UI shell (all missing)

| Item | Notes |
|---|---|
| Client screen state machine | Bevy `States` `ClientScreen` + screen plugin composition; the current `StatesPlugin` import already exists |
| Menu navigation model | Focus-ring widgets (controller) + mouse; shared confirm/cancel/back handling via the existing abstract actions; no gameplay-input coupling |
| UI component set | Buttons, list rows, panels, cards, budget bar, pips, focus ring; consistent with the doc-11 look; icons from the doc-12 HUD atlas |
| Screen scaffolds | Title, ServerSelect, Connecting, GameSelect, BuildEditor, Queue, Match HUD shell, Menu, Results, Settings, Error |
| Local persistence | Favorites, last-used name, settings file; follows the manifest/licensing discipline; keep out of the server graph |

### B. Session and protocol (partly have, partly missing)

| Item | Status | Notes |
|---|---|---|
| Non-fatal join lifecycle | Debug-only → change | `Rejected`/`Disconnected`/timeout currently write `AppExit::error()` (`session.rs`); must transition to Error screen instead; headless paths keep exiting |
| Display name | Missing, **design gap** | No name exists anywhere; needs a protocol decision (client name in `ClientHello` or a session message) and roster/pool replication/display — protocol fingerprint change |
| Game-type advertisement | Missing | New server→client message (or handshake payload) listing game types: name, mode, maps, settings, formation size; also carried pre-join by the registry heartbeat |
| Queue join/leave + pool state | Missing | Client requests pool membership (with game type + build); server replicates pool composition (players, names, builds, formation progress); formation/start is server-owned |
| Leave-match intent | Missing, **design gap** | Clean intentional departure message vs. plain disconnect; server already handles departure/forfeit |
| Address entry (`IP:port`, `hostname(:port)`) | Missing | Validate before connecting; hostname resolution with an optional default port; reuse the existing `Authentication::Manual` join path |
| Favorites (TOML) | Missing | Plain `name`/`addr` list, auto-saved on join/edit, hand-editable; same manifest discipline as assets; keep out of the server graph |
| Server metadata for browsing | Missing, **design gap** | Name, game types, mode, maps, settings, players, version, fingerprints; post-join from the advertisement, pre-join from a future registry heartbeat — schema/service scope is a later design decision, not matchmaking |
| Registry discovery service | Future | Heartbeats from dedicated servers to a registration endpoint; client browse tab; see `GAP-NET-INTERNET` for reachability caveats |
| Pool → match formation | Missing | Server-side pool accounting per game type, formation trigger (size vs. timer), team assignment, overflow stay-queued, disconnect handling; formed matches run concurrently (see the concurrent-matches design below) |
| Formation/start semantics | Design gap | Replace or reuse the existing all-ready countdown; ready-check vs. immediate formation; see underspecified list |
| Team size / player expectations | Design gap | Mode defines topology (2v2 MVP, 3v3 target); game types advertise formation size; server capacity vs. mode requirement must not conflict |

### C. Game selection, queue, and build editor (data partly exists; presentation missing)

| Item | Status | Notes |
|---|---|---|
| GameSelect screen | Missing | Game-type cards from the server advertisement (mode + maps + settings + pool size); mode filter tab |
| Queue screen | Missing | Pool composition from new replication: players, names, builds, `queued/needed` formation progress; Cancel leaves the pool |
| Build editor screen | Debug-only → replace | Replace `update_build_selection_overlay`; catalog, `resolve_build_recipe`, budget math, and preset profiles all exist and stay server-validated |
| Custom-recipe fields | Have | Power/reach/magazine/ultimate/passives with `PulsePower/Reach/Magazine` enums; present as labeled choices, not `{:?}` debug text |
| Build preview | Have | Resolved loadout + total points; present as stats/budget bar |
| Ready / restart-ready actions | Have, may retire | `MatchCommand::SetReady/ReadyForRestart` keep their wire; pool formation may make ready redundant and restart quorum a design decision |
| Build persistence | Design gap | Session-scoped today; local saved builds are `FUT-ARSENAL` (post-v1) but a session-local "my current build" card is implied |

### D. In-match presentation (mostly have, must be redesigned)

| Item | Status | Notes |
|---|---|---|
| Clean combat HUD | Debug-only → replace | Health/ammo/weapon/ultimate as icons+bars (doc 12 icons); team score as pips/objective bar; no roster text dump (scoreboard moves to the Menu overlay/View) |
| Crosshair + range feedback | Have | Preview geometry exists (`combat/client.rs`); align with doc-05 controller readability |
| Landing indicator | Have | Lobbed-weapon landing indicator exists |
| Objective presentation | Have | Hot Zone ring/fill (SDF), Wipeout scores; present per doc 11 area-effect rules |
| Scoreboard | Have (text) | View-button overlay; restyle as panel |
| Menu overlay | Debug-only → replace | Exists as pause overlay + controls text; becomes the Menu screen with Settings entry; the match continues underneath |
| Results screen | Debug-only → replace | Phase overlay text → proper screen with restart/leave |
| Debug overlay | M11 scope | Keep behind a toggle, never mixed into the play HUD |
| Damage/status readability | Have | Hit flash, hit confirmation, defeat feedback exist |

### E. Settings, credits, and error surfaces (missing; backlog-acknowledged)

- Input remap/calibration UI for the M11 settings resource (`GAP-UI-SETTINGS`).
- Audio and display settings (`GAP-AUDIO-SETTINGS`, `GAP-UI-WINDOW`).
- Credits screen for CC0/CC-BY attribution (`GAP-LEGAL-CREDITS`).
- Error presentation bound to M11 structured failure categories, in plain language.

### F. Verification

- Pure/UI tests: screen transitions, focus navigation, budget bar, error mapping;
- schedule tests: screen systems do not touch authoritative simulation; pool join/leave/formation;
- network tests: connect → game select → queue → formation → match → results → re-queue and
  join → reject → error → rejoin loops; pool formation with overflow, disconnect in queue, and
  disconnect in match; headless automation and existing exit-code contracts unchanged;
- visual/controller checks per M11 matrix at the supported layouts.

### G. Concurrent matches per process (design)

This section describes the intended design; it is not a commitment to build it. Ordering and gap
analysis happen later. The model re-instances the existing single-match machinery rather than
writing a parallel one. The replication part of this design depends on `GAP-NET-ROOMS` (Lightyear
room interest management):

| Item | Notes |
|---|---|
| Per-match rules and capacity | `MatchLifecycleRules`/`ResolvedMatchCapacity` become per-match state keyed by match id; every consumer (lifecycle, spawns, terrain admission, telemetry) reads the owning match's instance |
| Per-match mode | One process hosts heterogeneous matches; mode selection moves from app-install time (`install_server_game_mode`) to match-formation time |
| Per-match map and terrain | Per-match resolved map + terrain instance; M10 chunk/collider/brush budgets become per-match ceilings with a process admission ceiling so N matches cannot stack past the engine's verified capacity |
| Replication rooms | Lightyear `RoomPlugin`; a client receives only its own match's replication set; cross-match isolation is a correctness requirement, not an optimization |
| Admission by match id | Join targets a formed match (pool → match); per-match capacity admission; rejection remains explicit and documented |
| Fixed-tick budgets | All matches share one fixed tick; systems iterate per match instance; performance fixtures cover N concurrent matches at the 24-fighter ceiling |
| Telemetry and evidence | Per-match retention/summaries (already `MatchId`-scoped in places); global bounded histories stay bounded under concurrency |
| Server configuration | Game types become a config list (drafted TOML above); operator validation covers mode ↔ map ↔ capacity per type |
| Test strategy | Multi-pool formation, concurrent matches, cross-match isolation, overflow, disconnect-in-queue, and soak loops become first-class fixtures |

### H. Control-plane protocol (research item, no decision)

Open question worth investigating before the queue/build-session protocol is locked: should
session-level actions — player setup, build selection and, later, arsenal/inventory management —
travel over the gameplay protocol or a sideband classical REST channel?

| Dimension | Gameplay protocol (Lightyear session channel) | Sideband REST/HTTP |
|---|---|---|
| Transport | Reuses the game connection, auth, and reliability | Second transport: separate port/endpoint, TLS story, client HTTP stack in Bevy |
| Lifecycle | Actions die with the game connection; menu actions require a live game session | Survives/independent of game sessions; natural fit for pre-join setup |
| Semantics | Lightyear message request/response and validation patterns | Standard HTTP idempotency, caching, retries, JSON schema tooling |
| Coordination | Single source of truth for session state | Two channels must be reconciled (e.g., build chosen over REST must bind to a game session) |
| Boundary | One protocol to maintain, one fingerprint | Cleaner separation of control vs. gameplay authority; fits future services (accounts, inventory) |

Investigation should decide one coherent model, record the choice in this document, and then lock
the queue/build protocol. The current prototype is entirely in-band (build selection, match
commands), so in-band remains the default unless the investigation finds a concrete boundary
problem.

## Underspecified or missing in the design docs

1. **Player display name.** No design anywhere; identity is a numeric client id. Needs a protocol
   and roster/pool decision before any queue can show names.
2. **Pool formation rules.** Does a match form at the game type's maximum size, its minimum plus a
   grace timer, or a ready-check (accept/decline) after formation? Overflow behavior (stay queued)
   is decided; timing is not. Team assignment from a mixed pool (vs. pre-formed groups) needs a
   rule; groups/parties are out of scope but two friends queuing together is plausible.
3. **Restart and rematch semantics.** Pool formation supersedes all-ready start. Does the existing
   restart quorum survive as an in-group rematch, or does every completed match return everyone to
   their pools? Both are defensible; pick one before implementation.
4. **Build selection timing.** The current flow selects builds inside the waiting phase over a
   fighter. The pool flow selects before queuing; the server must accept a build selection from a
   queued (not yet matched) player and carry it into the formed match.
5. **Registry service scope.** The heartbeat schema (name, addr, game types, mode, maps, settings,
   players, version, protocol/content fingerprints), registration endpoint, refresh cadence,
   stale-server expiry, and client browse UI are undesigned. It is a later service and must stay
   clearly distinct from matchmaking; it also does not solve reachability (NAT/relay).
6. **Favorites file location and format.** TOML list is decided; file path, auto-save timing, and
   hand-edit behavior need a small spec (no settings persistence exists yet).
7. **Server game-type configuration.** TOML shape is drafted above; validation (map ↔ mode
   compatibility, capacity vs. map spawns, formation size bounds) and whether game types are
   immutable per process need a spec.
8. **Leaving and returning.** There is no design for intentional departure from a pool or match,
   return-to-menu, or what the server does with a leaving queued player (drop from pool) beyond the
   existing forfeit rules for matches.
9. **Local build persistence.** Session-scoped by default; `FUT-ARSENAL` covers accounts, but a
   simple local "last used build" is unstated and cheap.
10. **Bot practice.** Doc 08 explicitly permits an authoritative bot-practice mode (`GAP-MODE-TRAINING`),
    but no menu path ("Play vs Bots") is designed and v1 bots are match-fillers only. A bot-practice
    game type is a server-side concern (hosted or local), not a client-hosted game.
11. **Error taxonomy UX.** Structured failure categories exist as M11 work, but the mapping from
    each category to a screen/retry path is not designed.
12. **Credits and settings placement.** Screens are named in the backlog; where they live in the
    navigation graph is not.
13. **Control-plane protocol.** See section H: in-band gameplay protocol vs. sideband REST for
    player setup, build selection, and future inventory/arsenal actions is an open investigation;
    in-band remains the default until it concludes.
