# Canonical cross-version candidate index

Last reconciled: **2026-08-26**, after V11 completed and accepted playable deterministic Practice
bots, objective-priority and perimeter-recovery feedback corrections, affected verification, and
closeout.

This file is the canonical index of unresolved product and technical candidates for future PewPew
Blitz versions. It provides one place to compare candidates without copying the detailed research,
specifications, evidence, or historical rationale retained by completed version roadmaps and design
documents.

A candidate is not an implementation commitment or a permanent priority. It becomes active only
when promoted into the next version roadmap and processed through research and user specification
review. Completed roadmap rows remain historical sources of truth and are not rewritten from this
index.

## How to use this index

- Select future work by current player value, product direction, dependency evidence, and delivery
  cost—not by the age or identifier of a candidate.
- Promote the smallest player-visible vertical slice. A parent candidate may produce one focused
  milestone without committing every capability named by its source documents.
- Create the next version roadmap and first milestone only when that version is intentionally
  chosen. Record research, exact scope, architecture, tests, playtest, feedback, and closeout there.
- Update this index when a candidate is promoted, split, superseded, resolved, rejected, or gains a
  concrete promotion trigger.
- Keep catalog-scale idea inventories in their owning design documents. Add a separate candidate
  here only when it represents a plausible version outcome or a concrete maintenance obligation.

## Candidate states

| State | Meaning |
|---|---|
| Candidate | Eligible for future-version research; no order or delivery commitment is assigned |
| Partially delivered | A useful baseline exists, but a separately valuable remainder is unresolved |
| Trigger-bound | Revisit only when the stated product, platform, deployment, or evidence condition occurs |
| Promoted | Owned by an active version roadmap; that roadmap is the implementation scope contract |
| Resolved | Delivered, superseded, or explicitly closed; retained only in the archive below |

## Player-visible product candidates

| ID | Candidate outcome | State and promotion trigger | Detailed sources and dependencies |
|---|---|---|---|
| CAND-BOTS | Playable, readable server-hosted opponents in the existing Start Practice flow | Resolved by completed V11 M01; broader bot builds, difficulty, avoidance, tactics, hosting, and learned-policy work retain their explicit V11 backlog gates | [V11 roadmap](./implementation/v11/roadmap.md); [V11 M01 closeout](./implementation/v11/milestone-01.md); [bot decision and first-slice contract](./10-bots.md) |
| CAND-RELEASE-POLISH | Release-quality controller feel, audio, HUD/combat/terrain/objective readability, balance, match length, and mode pacing | Candidate; promote before claiming release readiness or when focused playtests identify the next highest-value feel problem | [`POST-V1-RELEASE-POLISH`](./implementation/v1/roadmap.md#v1-backlog); [V2 manual-matrix rows](./implementation/v2/roadmap.md#v2-backlog); [V10 closeout](./implementation/v10/roadmap.md) |
| CAND-ARSENAL | Persistent player-owned brawlers, immutable fighter-profile/weapon-base creation choices, four interchangeable owned weapon-part slots, production editing, and the minimum account/storage boundary required by that loop | Resolved by completed V7; acquisition, currencies, loot, purchases, trade, crafting, global accounts, and cloud profiles were outside that outcome and require a separately promoted player loop | [V7 roadmap](./implementation/v7/roadmap.md); [fighter model](./02-fighter-model.md); [weapons and abilities](./03-weapons-and-abilities.md) |
| CAND-WEB-ARSENAL | A browser profile/dashboard that creates, edits, deletes, selects, and presents the same server-owned brawlers through an HTTP API | Trigger-bound; promote when browser or remote arsenal management becomes a selected player workflow. It must call the same profile commands, revisions, validation, and queue-time edit rejection as the native Dashboard and must not access SQLite directly or imply browser gameplay | [V7 roadmap](./implementation/v7/roadmap.md); [player UX](./13-player-ux.md); [multiplayer server architecture](./14-multiplayer-server-architecture.md) |
| CAND-WEB-GAME-CLIENT | A supported Bevy WASM gameplay client connecting through a browser-compatible Lightyear transport while preserving the routed, server-authoritative match path | Trigger-bound; promote only when browser gameplay is a supported platform target. Requires a WebTransport or evaluated WebSocket ingress rather than raw UDP, WASM-specific client feature composition and persistence, certificate/deployment policy, browser input/audio/render checks, and cross-browser performance evidence | [network architecture](./08-network-architecture.md); [multiplayer server architecture](./14-multiplayer-server-architecture.md); [current native-only transport features](../Cargo.toml); [local Lightyear WASM/WebTransport example](../references/lightyear/examples/simple_setup/README.md) |
| CAND-COMBAT-CAPABILITIES | One new readable combat/build family, such as support/control payloads, a new ultimate, systemic status interaction, an advanced projectile, or a terrain-destruction carrier | Candidate parent; promote exactly one coherent capability family after choosing its player-visible build tradeoff | [fighter model](./02-fighter-model.md); [weapons and abilities](./03-weapons-and-abilities.md); [future-version combat candidates](./implementation/v1/roadmap.md#future-version-candidate-backlog); [`GAP-DESIGN-TERRAIN-RESERVATION`](./implementation/v1/roadmap.md#v1-backlog) |
| CAND-GAME-MODE | One complete additional authoritative mode with compatible map requirements and a player-visible loop | Partially delivered: V10 completed the focused `HeistModePlugin`. Gem Grab, Solo Showdown, and another complete mode remain candidates rather than framework requirements | [V10 roadmap](./implementation/v10/roadmap.md); [damageable objects and Heist specification](./18-damageable-world-objects-and-heist.md); [future-version mode candidates](./implementation/v1/roadmap.md#future-version-candidate-backlog); [maps and modes](./04-maps-and-game-modes.md) |
| CAND-ENVIRONMENT-GAMEPLAY | One readable environment slice—surface, concealment, hazard, traversal device, interactive geometry, or ability-created region—using server-owned outcomes | Partially delivered: V9 completed concealment/reveal and V10 completed oil barrels plus one treasure-chest/restoration-pickup behavior. Unrelated hazards, traversal devices, interactions, movement surfaces, and temporary regions remain candidates | [V10 roadmap](./implementation/v10/roadmap.md); [damageable objects and Heist specification](./18-damageable-world-objects-and-heist.md); [V9 roadmap](./implementation/v9/roadmap.md); [concealment and reveal specification](./17-concealment.md); [environment gameplay direction](./09-environment-gameplay.md) |
| CAND-MAP-CONTENT | Additional built-in maps or themes using the implemented sparse-grid map-asset contract | V10 consolidated product-visible feature proofs into the Feature Yard integration family. Promote a later map slice only to deliberately author and playtest fun/balanced player content or a genuinely new theme, not another isolated mechanic fixture | [V10 Feature Yard closeout](./implementation/v10/milestone-02b.md); [grid map-asset specification](./16-grid-map-asset-system.md); [maps and modes](./04-maps-and-game-modes.md) |
| CAND-MAP-BUILDER | A player-facing workflow for editing, validating, saving, reopening, and launching bounded map recipes without authoring executable mode rules | Candidate; promote after choosing a deliberately bounded authoring vertical slice. Publishing, discovery, moderation, arbitrary assets, and platform services remain separate decisions | [`FUT-MAP-BUILDER`](./implementation/v1/roadmap.md#v1-backlog); [creator direction](./00-product-direction.md#creator-direction); [V4 storage/object evidence](./implementation/v4/roadmap.md) |
| CAND-MAP-PROVISIONING | Server-selected map-bundle delivery and caching so clients need not embed the complete map library | Trigger-bound; research before server-hosted custom maps or when built-in library size becomes a measured distribution problem | [V4 M02 map-document architecture](./implementation/v4/milestone-02.md); [network map authority](./08-network-architecture.md#map-recipe-and-mode-authority). Coordinate bundle identity, assets, trust, cache/recovery, and global compatibility as one content-delivery boundary |
| CAND-ORIGINAL-ART | A coherent original PewPew Blitz production-art replacement or extension for the dashboard, fighters, environments, weapons, animation, and effects | Trigger-bound; promote only with an art-production budget and a coherent replacement target | [`V3-ORIGINAL-ASSETS`](./implementation/v3/roadmap.md#v3-backlog); [`V5-ORIGINAL-DASHBOARD-ART`](./implementation/v5/roadmap.md#initial-v5-backlog); [art, presentation, and asset specification](./11-art-and-presentation-direction.md) |
| CAND-RELEASE-READINESS | Remaining desktop release work: broader minimum-spec coverage, colorblind palette, explicit resolution/frame-limit/cursor policy, non-Xbox glyph/rumble support, localization decision, packaging/notarization, and battery/thermal behavior | Trigger-bound; split into the smallest required slices when a supported-platform or external-distribution gate is chosen | [V1 explicit release exclusions](./implementation/v1/roadmap.md#explicitly-outside-v1); [player UX settings/accessibility contract](./13-player-ux.md); [V3 performance baseline](./implementation/v3/roadmap.md); [V5 closeout limitations](./implementation/v5/roadmap.md#v5-closeout) |

## Network, service, and session candidates

| ID | Candidate outcome | State and promotion trigger | Detailed sources and dependencies |
|---|---|---|---|
| CAND-INTERNET-REACHABILITY | Supported play beyond LAN through an explicit port-forwarding, NAT-traversal, relay, or hosted-connectivity policy | Trigger-bound; promote when internet play becomes a supported product target | [V2 hosting deferrals](./implementation/v2/roadmap.md#explicitly-deferred-beyond-v2); [network architecture](./08-network-architecture.md); [multiplayer server architecture](./14-multiplayer-server-architecture.md) |
| CAND-SERVER-DISCOVERY | A bounded server registry with authenticated/current advertisements, expiry, refresh, and client browsing while preserving direct hostname/IP entry and local favorites | Trigger-bound; promote with public multi-server discovery and coordinate with internet reachability because discovery does not make a server reachable | [player UX server-selection direction](./13-player-ux.md); [V2 hosting deferrals](./implementation/v2/roadmap.md#explicitly-deferred-beyond-v2) |
| CAND-PREDICTION-LAG-COMP | Measured owner prediction and/or lag compensation that improves supported latency without violating terrain, collision, authority, or readability | Trigger-bound; revisit with a terrain-aware prediction candidate or latency evidence that current authoritative/interpolated play is inadequate | [`M03-PRED`](./implementation/v1/roadmap.md#v1-backlog); [network architecture](./08-network-architecture.md); [local prediction experiment](../src/client/prediction.rs) |
| CAND-SESSION-CONTINUITY | One explicit continuity feature such as interrupted-match resumption, join-in-progress, or a spectator/observer client | Trigger-bound; promote one feature when continuity, tournament observation, or larger playtest operations demonstrate the need | [`V2-ROUTE-RESUMPTION`](./implementation/v2/roadmap.md#v2-backlog); [V2 explicit deferrals](./implementation/v2/roadmap.md#explicitly-deferred-beyond-v2); [player UX](./13-player-ux.md) |
| CAND-HOSTING-HARDENING | Internet-facing capacity, credentials, security, fleet scheduling, autoscaling, moderation, and administration appropriate to a concrete deployment | Trigger-bound; do not build a generic backend before a public hosting target exists | [`V2-HOSTING-HARDENING`](./implementation/v2/roadmap.md#v2-backlog); [V2 explicit deferrals](./implementation/v2/roadmap.md#explicitly-deferred-beyond-v2); [multiplayer server architecture](./14-multiplayer-server-architecture.md) |

Global accounts, cloud profiles, social systems, parties, ranked play, currencies, monetization,
live operations, and mobile controls remain outside the current candidate index unless a future
product decision promotes a concrete player outcome. V7 owns only the minimum server-side identity,
storage, and entitlement boundary required by its persistent-arsenal slice.

## Trigger-bound technical maintenance

These rows do not compete with player-visible candidates for permanent priority. Promote them when
their trigger occurs or include them in a version whose changed surface makes the maintenance
necessary.

| ID | Obligation | Promotion trigger and source |
|---|---|---|
| MAINT-ROUTED-HARDENING | Re-measure and optimize routed IPC/egress, MTU, CPU, and capacity behavior | A real deployment target or measured bottleneck; [`V2-ROUTED-HARDENING`](./implementation/v2/roadmap.md#v2-backlog) |
| MAINT-WINDOWS-IPC | Implement the production Windows named-pipe backend | Windows becomes an active supported target; [`V2-WINDOWS-IPC`](./implementation/v2/roadmap.md#v2-backlog) |
| MAINT-TRANSPORT-CONTINGENCY | Reconsider bounded worker-port Lightyear UDP instead of routed transport | Only a qualifying routed hard-gate failure and explicit user approval; [`V2-TRANSPORT-CONTINGENCY`](./implementation/v2/roadmap.md#v2-backlog) |
| MAINT-LEGACY-DIRECT-UDP | Retire the V1 direct-UDP scripts and documentation | Their explicit comparison/debug value is gone; [`V2-V1-DIRECT-UDP-RETIREMENT`](./implementation/v2/roadmap.md#v2-backlog) |
| MAINT-LEGACY-BUILD-SYSTEM | Remove the superseded full-build preset system now that server-owned saved brawlers are the sole product loadout workflow | Promote as a focused cleanup version or milestone before adding more content to the legacy path. Remove named full-build presets and their catalog IDs/recipes, the unreachable Build Editor and standalone build persistence, the non-product direct-session selection state/protocol/authority path where no supported diagnostic still requires it, and preset-only telemetry/configuration. Convert automation and network fixtures to saved brawlers or explicit canonical recipes. Preserve active weapon bases/weapon presets, ultimate and passive definitions, immutable resolved loadouts, profile persistence, routed admission, and server authority. Advance affected compatibility schemas and fail closed rather than retaining decoders. Source: V7 superseded this path; V9 M03 exposed the dormant `Veilkeeper` preset while adding Concealment Field |
| MAINT-COMBAT-PROFILES | Repair, replace with routed evidence, or retire the documented legacy `network-combat-profiles` gate | Before relying on it for evidence or when retiring the legacy direct-UDP path; the historical failure predates terrain work and requires fresh reproduction |
| MAINT-NETWORK-TEST-LINT | Decide and enforce a Clippy policy for the `network-test` test/performance configuration | When test-code warning cleanliness becomes a CI/release gate; current CI runs tests but not a `-D warnings` Clippy gate for this feature |
| MAINT-LIFECYCLE-SOAK | Re-run simultaneous heterogeneous-mode and repeated completion/requeue bounded-growth campaigns | Host scale, lifecycle changes, or leak evidence; [`V2-M06-LIFECYCLE-SOAK`](./implementation/v2/roadmap.md#v2-backlog) |

## Dependency and scoping rules

- **Bots:** begin with the server-hosted practice vertical slice. External bot clients, automatic
  multiplayer substitution, and supervisor-managed bot processes are later decisions.
- **Arsenal and equipment:** saved brawler identity and owned item instances are distinct. Promote
  only the acquisition, entitlement, persistence, and UI behavior required by the selected slice.
- **Browser surfaces:** a web arsenal is an HTTP adapter over profile authority and does not require
  a browser game client. A WASM game client is a separate platform/transport effort and cannot use
  the current raw-UDP ingress.
- **Combat and environment:** add one player-visible capability before extracting general status,
  region, summon, payload, or navigation frameworks.
- **Map builder and provisioning:** the builder can prove bounded authoring over current embedded
  catalogs; provisioning becomes mandatory only before server-hosted custom-map distribution or a
  measured thin-client need.
- **Internet reachability and discovery:** a registry lists servers but does not solve NAT or relay.
  Specify their deployment and trust boundaries together when both enter scope.
- **Prediction and lag compensation:** neither is automatically required by internet play. Adopt
  only after impairment evidence identifies a player-visible problem and the candidate preserves
  server authority and terrain correctness.
- **Release readiness:** do not turn the complete platform matrix into one milestone. Promote the
  smallest platform, accessibility, packaging, or performance gate needed for the intended handoff.

## Resolved and superseded archive

| Historical item | Resolution and evidence |
|---|---|
| Settings screen and local persistence (`GAP-UI-SETTINGS`) | Resolved by V2; V5 retains persisted input, UI scale, reduced motion/effects, master volume, focus mute, fullscreen, and vsync through the dashboard shell. See [V2 roadmap](./implementation/v2/roadmap.md) and [V5 closeout](./implementation/v5/roadmap.md#v5-closeout) |
| Authoritative practice mode (`GAP-MODE-TRAINING`) | Resolved by V2 with routed server-hosted practice and manifest fillers; completed V11 added playable server-hosted controller behavior without reopening practice architecture |
| In-match Pause naming (`GAP-UI-PAUSE-RENAME`) | Resolved by V2: the non-pausing menu context is explicit; the physical pause/menu action remains an input label |
| Cross-match Lightyear Rooms (`GAP-NET-ROOMS`) | Superseded by completed process-per-match routing. Reopen only for measured within-match visibility or a future multi-match authority |
| Terrain module ownership (`GAP-ORG-TERRAIN-SPLITS`) | Resolved by V1 M11; presentation, recovery/convergence, lifecycle, and network ownership were split while preserving public API and schedule contracts. See [`M10-ORG-TERRAIN-SPLITS`](./implementation/v1/roadmap.md#v1-backlog) |
| Baseline client render targets (`GAP-PERF-CLIENT`) | Partially resolved by V3/V5 native frame-time and lifecycle evidence. Broader device/minimum-spec coverage remains under `CAND-RELEASE-READINESS` |
| Baseline 3D combat presentation (`GAP-FX-PRESENTATION`) | Partially resolved by V3 muzzle, impact, damage, reset, trail, status, and debris work. Rich authored presentation remains under `CAND-ORIGINAL-ART` or a focused combat slice |
| Additional built-in proof (`GAP-MAPS-BUILTIN`) | V4 added a second independent map/theme; V10 consolidated product-visible feature proofs into Feature Yard. Proper fun/balanced map content remains `CAND-MAP-CONTENT` |
| Additional authoritative mode / Heist (`GAP-OBJ-DELIVERY`) | Resolved for the first additional mode by completed V10 Heist. Gem Grab, Solo Showdown, and other complete loops remain under `CAND-GAME-MODE` |
| Environmental damage source (`M08-ENV-SOURCE`) | Resolved by V10 oil-barrel attribution with bounded immediate-object cause and initiating-player lineage. Other environment families remain under `CAND-ENVIRONMENT-GAMEPLAY` |
| Credits and asset attribution (`GAP-LEGAL-CREDITS`) | Resolved by V5: Credits is reachable from the Dashboard menu, current Kenney assets are identified as CC0, and full asset license texts ship under `assets/licenses/` |
| Font licensing (`GAP-LEGAL-FONTS`) | Resolved by V5 with shipped Fira Mono and Lilita One SIL OFL license texts and Credits attribution |
| Focus-loss audio (`GAP-AUDIO-FOCUS`) | Resolved by persisted mute-when-unfocused behavior; broader audio work remains `CAND-RELEASE-POLISH` |
| Fullscreen/vsync baseline (`GAP-UI-WINDOW`) | Partially resolved by persisted fullscreen, vsync, and UI-scale settings. Explicit resolution, frame-limit, and cursor-capture policy remains under `CAND-RELEASE-READINESS` |
| Audio-settings baseline (`GAP-AUDIO-SETTINGS`) | Partially resolved by persisted master volume and focus mute. Separate SFX/music categories and actual music remain optional release-polish slices |
| V5 dashboard planning rows | Preferred-server policy, factual dashboard information, preview composition, results actions, and branding promotion were resolved by V5 M01–M03. See [V5 backlog and closeout](./implementation/v5/roadmap.md#initial-v5-backlog) |

## Legacy gap-ID mapping

The pre-V5 documentation audit used `GAP-*` identifiers. This table preserves their disposition
without keeping resolved findings in active candidate tables or duplicating their old descriptions.

| Canonical candidate or archive | Historical IDs |
|---|---|
| CAND-INTERNET-REACHABILITY | `GAP-NET-INTERNET` |
| CAND-SERVER-DISCOVERY | `GAP-NET-REGISTRY` |
| CAND-COMBAT-CAPABILITIES | `GAP-COMBAT-SUPPORT`, `GAP-ABILITY-ULTIMATES`, `GAP-FIGHTER-ATTRS`, `GAP-WEAPON-MECH`, `GAP-TERRAIN-BEAM`, `GAP-DESIGN-TERRAIN-RESERVATION` |
| CAND-ARSENAL | `GAP-ITEM-EQUIPMENT`, `FUT-ARSENAL` |
| CAND-RELEASE-POLISH | `GAP-AUDIO-SETTINGS`, `GAP-UI-COLORBLIND`, `GAP-INPUT-AIMASSIST`, `GAP-INPUT-DEVICES`, `POST-V1-RELEASE-POLISH` |
| CAND-RELEASE-READINESS | `GAP-PERF-CLIENT`, `GAP-BUILD-NOTARIZE`, `GAP-UI-WINDOW`, `GAP-I18N`, `GAP-PLATFORM-BATTERY` |
| CAND-MAP-CONTENT | `GAP-MAPS-BUILTIN` |
| CAND-MAP-BUILDER | `GAP-MAP-EDITOR`, `FUT-MAP-BUILDER` |
| CAND-MAP-PROVISIONING | `GAP-MAP-PROVISIONING` |
| CAND-ORIGINAL-ART | `GAP-COMBAT-DEBRIS`, `GAP-FX-PRESENTATION`, `V3-ORIGINAL-ASSETS`, `V5-ORIGINAL-DASHBOARD-ART` |
| CAND-GAME-MODE | Gem Grab, Solo Showdown, and other future complete modes; `GAP-OBJ-DELIVERY`/Heist is resolved by V10 |
| CAND-ENVIRONMENT-GAMEPLAY | `GAP-ENV-SLIPPERY`, `GAP-ENV-BENEFICIAL`, `GAP-ENV-TACTICAL`, `GAP-ENV-TRAVERSAL`, `GAP-ENV-INTERACTIVE`, `GAP-ENV-HAZARDS`, `GAP-ENV-WATER`, `GAP-ENV-AUTHORING`, `GAP-ENV-CONCEAL-DETAIL`, `GAP-REGIONS-ABILITY` |
| CAND-PREDICTION-LAG-COMP | `GAP-NET-LAGCOMP`, `M03-PRED` |
| CAND-SESSION-CONTINUITY | `GAP-TOOL-SPECTATE`, `V2-ROUTE-RESUMPTION` |
| Trigger-bound maintenance | `GAP-TOOL-COMBATPROFILES`, `GAP-TOOL-NETTEST-LINT` and the mapped `V2-*` maintenance rows above |
| Resolved/superseded archive | `GAP-UI-SETTINGS`, `GAP-NET-ROOMS`, `GAP-LEGAL-CREDITS`, `GAP-MODE-TRAINING`, `GAP-AUDIO-FOCUS`, `GAP-LEGAL-FONTS`, `GAP-UI-PAUSE-RENAME`, `GAP-ORG-TERRAIN-SPLITS` |

## Conditional and rejected directions

The following are not active candidates without new evidence or an explicit product decision:

- perspective/free/orbit camera after accepted orthographic play, general 3D physics, vertical
  gameplay, and advanced rendering without an owned art or performance need—see the
  [V3 backlog](./implementation/v3/roadmap.md#v3-backlog);
- a historical internal Brawler-to-PewPew-Blitz repository/crate/config migration without a concrete
  compatibility or maintenance benefit—see `V5-INTERNAL-NAME-MIGRATION` in the
  [V5 backlog](./implementation/v5/roadmap.md#initial-v5-backlog);
- procedural map generation, automatic balance generation, arbitrary user-authored executable mode
  rules, structural collapse, fluids, and material simulation;
- generic social, monetization, live-operations, anti-cheat, backend, AI, rendering, UI, or content
  frameworks before one selected product slice demonstrates the boundary.
