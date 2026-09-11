# Brawler repository guide

## Quick orientation

PewPew Blitz (repository and crate identifier `brawler`) is an original, cross-platform top-down
arena shooter built around player-authored fighter builds. Combat readability, meaningful build
tradeoffs, short matches, reusable content primitives, and server-authoritative networking are the
core product constraints.

For current work, start with `ticket list` and `ticket task <ID>`. Ticket descriptions and specs
own active scope, status, acceptance criteria, and evidence. `docs/tasks/` contains readable Ticket
mirrors but is never the agent write path.

Start with:

1. `docs/README.md` and `docs/00-product-direction.md` for the durable document map, product intent,
   and non-goals.
2. `docs/01-engine-decision.md` and `docs/08-network-architecture.md` for the current Bevy/ECS,
   authority, schedule, plugin, compatibility, registry, and content-fingerprint contracts.
3. `docs/02-fighter-model.md` and `docs/03-weapons-and-abilities.md` for authored definitions,
   immutable resolved loadouts, runtime capabilities, combat transactions, and extension rules.
4. `docs/04-maps-and-game-modes.md`, `docs/16-grid-map-asset-system.md`, and
   `docs/18-damageable-world-objects-and-heist.md` for maps, effect-tile capability projection,
   local mode/object-reaction registration, and environment authority.
5. `docs/10-bots.md` and `docs/17-concealment.md` for server-hosted Practice policy, semantic
   observations, fair delayed perception, and observer-specific replication.
6. `docs/11-art-and-presentation-direction.md` for renderer, audio/VFX request boundaries,
   readability, assets, provenance, and degradation.
7. `docs/13-player-ux.md`, `docs/14-multiplayer-server-architecture.md`, and
   `docs/15-balance-lab.md` for player flow, routed processes, and the current tuning contract.
8. `docs/implementation/v1/` through `docs/implementation/v12/` only when historical delivery,
   feedback, or evidence is relevant. Later draft version documents are context, not active scope.

V1 completed on 2026-08-18 as a server-authoritative gameplay MVP after the final basic user
playtest. It is not a release-ready claim: controller feel, audio, HUD/readability, balance, pacing,
and related tuning remain tracked as `POST-V1-RELEASE-POLISH`. V2 completed and was accepted on
2026-08-20. V3 completed and was accepted on 2026-08-20. M01 completed after the user accepted the 3D
feasibility result and its projectile-origin corrections. M02 completed on 2026-08-20 after its
default 3D arena/map/terrain/camera/input cutover, projectile-placement feedback fix, removal of the
obsolete projectile sprite/XY writer, affected verification, and user acceptance. M03 completed on
2026-08-20 after its independent fighter/combat visual implementation, canonical verification,
native smoke, and accepted playtest handoff. M04 completed after renderer retirement, lifecycle and
readability verification, iterative fighter-presentation feedback, documentation reconciliation,
and its learning review. V4 M01 completed after its accepted first presentation pass and learning
review. V4 M02 completed on 2026-08-21 after canonical verification and a user playtest confirmed
that the storage migration preserved the accepted presentation. V4 M03 and V4 completed on
2026-08-21 after Ashen Court and the second theme passed canonical/native verification, the
detached-overhead feedback fix was accepted, and the learning review was recorded. V5 completed and
was accepted on 2026-08-22 after auto-connect, the responsive Player Dashboard, connected-loop
convergence, recovery/lifecycle hardening, routed 1v1/2v2/3v3 E2E, and native Dashboard/gameplay
render evidence passed closeout.

V6 completed and was accepted on 2026-08-22 after the Balance Lab's routed Practice workflow,
validated persistence, isolation, and native operator checks passed. V7 completed and was accepted
on 2026-08-23 after persistent saved-brawler profiles and four-slot weapon-part equipment passed
storage, routed handoff, recovery, and player-flow verification. V8 completed and was accepted on
2026-08-23 after its hard map-system cutover, domain-organized active content, legacy removal, and
canonical/E2E/native verification. V9 completed and was accepted on 2026-08-24 after all three
concealment sources, observer-specific reveal, Reveal Scan, the server-authoritative brawler
catalog, saved-brawler UX remediation, full verification, native gameplay confirmation, and its
learning review passed closeout.

V10 completed and was accepted on 2026-08-25 after damageable oil barrels, mirrored Heist, the
Feature Yard Wipeout/Hot Zone/Heist 1v1/2v2/3v3 family, treasure chests, restoration pickups,
Balance Lab evolution, full routed/capacity/native verification, accepted presentation and point-
blank collision feedback, documentation reconciliation, and its learning review passed closeout.

V11 completed and was accepted on 2026-08-26 after playable deterministic server-hosted Practice
bots, fair delayed perception and concealment, bounded resumable navigation, Pulse/Dash combat,
Wipeout/Hot Zone/Heist objective behavior, full automated/routed/native verification, accepted
objective-priority and perimeter-recovery feedback corrections, documentation reconciliation, and
its learning review passed closeout.

V12 completed and was accepted on 2026-08-27 after its three purpose-built 3v3 maps, dynamic
map/viewport framing, matched one-cell fighter footprint, Balance Lab correctness and presentation,
server-authoritative health/ammunition recovery, instant paired evidence capture, projectile
geometry/readability correction, final gameplay balance pass, full verification, documentation
reconciliation, and learning review passed closeout.

V1–V12 are historical delivery records. Brawler no longer uses large numbered versions or
milestone roadmaps to plan, sequence, or report current progress. Existing later draft documents
may provide research or design context, but they do not establish active scope or status. Current
work is owned and tracked through the Ticket CLI process defined below.

## Technical stack

- The main Rust package provides independently buildable macOS-client and headless gameplay-worker
  configurations; `packages/brawler-routing` owns the completed V2 route/IPC protocol used by the
  supervisor, lobby worker, and match workers.
- Bevy 0.19 for ECS, application/plugin structure, client-side 3D world rendering, screen-space UI,
  input, assets, animation, and audio.
- Lightyear 0.29 for client/server transport, input networking, replication, interpolation, and later prediction/rollback where evidence justifies it.
- Avian 2D 0.7 for authoritative planar collision and map-asset colliders. V3 does not
  replace it with 3D physics.
- Fixed-tick, dedicated-server-authoritative simulation from the first gameplay code.
- macOS is the initial client development target; local dedicated-server and multi-client testing are required.

Use Bevy's `World` as the runtime gameplay model. Keep authored definitions, selected builds, runtime ECS state, networking registration, and client presentation distinct without assuming that each concern needs a crate or architectural layer. The dedicated-server configuration must exclude rendering, windowing, audio, device input, and client assets. Networked types use stable player, match, and definition IDs rather than exposing process-local ECS entity identity across the wire.

Group focused systems, components, resources, and messages into cohesive plugins. Share a gameplay system between server and client only when it genuinely executes in both places, such as measured client prediction. Keep server-only authority rules on the server and presentation systems on the client. Create another package or public API only for a demonstrated feature-isolation, platform, compile-time, testing, or reuse boundary.

For architecture decisions, prioritize Brawler's gameplay and authority requirements, the local Lightyear material, verified Bevy 0.19 APIs, and Bevy-native patterns before general Rust architecture advice. Server-oriented DDD or hexagonal architecture is not the governing model; use ports/adapters only at a concrete external boundary where they solve an observed problem.

## Current source layout

The crate keeps one public gameplay/application API while organizing implementation by ECS state
ownership, execution role, plugin composition, and schedule phase:

```text
src/
  lib.rs; gameplay.rs; protocol.rs; config.rs; content.rs; timing.rs
                           crate gates, fixed schedules, wire boundary, validated config,
                           contributor-sealed gameplay content, and timing policy
  bin/{client,server,profile_admin}.rs
                           thin executable entry points
  abilities/               activation/charge plus focused Dash, Sentry, cloak/reveal,
                           demolition, elemental-field, Big Blob, passive, telemetry ownership
  bots/                    private Practice policy, behavior registry, observations, navigation,
                           team/objective projections, diagnostics, and tests
  builds/                  authored catalog, shared passive/ultimate bounds, loadout resolution
  combat/                  definitions/model/cues/outcomes/telemetry/evidence; attack/emission;
                           delivery/projectiles; staged effect planning/application/runtime;
                           combat-owned environment damage; client/server adapters
  concealment/             authored rules, fields, observer-specific relevance, telemetry
  map/                     catalog/model/resolution/effect-tile capabilities/client/server
    runtime/               installation, dynamics, effect-tile authority, object authority,
                           and sealed terminal-reaction registry
  modes/                   bounded local registry plus Wipeout/Hot Zone/Heist registrations
  matchplay/               common lifecycle/spawns/server plus focused mode rule plugins
  movement/                input shaping, authority, arena geometry, and planar collision
  profiles/                saved-brawler model/catalog/server authority/SQLite storage
  weapon_parts/            authored definitions, canonical modifiers, deterministic resolver
  client/
    flow/                  ordered action/observation/commit composition; profile, equipment,
                           and match-flow reducers plus screen-owned presentation
    queue/                 matchmaking, Practice, loading, and automation
    session/               routed connection/admission/transitions/commands/observation
    audio/                 authored mappings, producer registry, semantic requests, playback
    vfx/                   authored profiles, producer registry, semantic requests
    presentation_3d/       sole world renderer, combat/environment, object-health projection
    settings/              persisted calibration/bindings/UI
    *.rs                   assets, input, HUD, shell, profile, targeting, and evidence adapters
  server/
    admission.rs           manifest validation and pure match-join planning
    fighter_spawn.rs       shared controller-neutral authoritative fighter assembly
    lobby/                 catalog/queue plus profile transaction adapter
    balance_lab/           HTTP/persistence/roster, typed editor adapters, staged transaction
    practice.rs; routed_worker.rs; worker.rs; verification.rs; mod.rs
  diagnostics/             process closeout/identity/sampling plus failure and client overlay
packages/brawler-routing/  route envelope, IPC, supervisor/worker protocol
content/{catalogs,maps}/   headless-safe authored gameplay data
assets/catalogs/           client-only audio/VFX/map presentation data
tools/balance-lab-web/     TypeScript operator application
tests/network/             separate-App authority/replication scenarios and reusable harness
tests/performance.rs       fixed-tick and subsystem performance/capacity gates
```

`content/catalogs/` owns build-embedded, headless-safe gameplay definitions. `content/maps/` owns
the built-in map index and one sparse map-asset recipe per built-in. `assets/catalogs/` owns
client-only visual paths and presentation data. Schema versions live in the documents and Rust
compatibility constants, not directory names. `references/` contains read-only upstream material
and is not part of Brawler's production module layout.

The routed supervisor, route envelope, IPC transport, and isolated lobby/match-worker composition
are completed V2 production paths. `just server`, `just client`, `just run`, and `just e2e` exercise
that routed topology; `scripts/network.sh` remains only the explicitly named legacy direct-UDP
diagnostic baseline. V3's `WorldPresentationPlugin` is the sole gameplay-world renderer; the
primitive override validates 3D degradation and is not a renderer selector.

## Code organization rules

- Treat each `mod.rs` as a composition and intentional public-API surface, not an implementation
  dumping ground. It may define shared system sets/resources, install plugins/schedules, and
  re-export the small API used by sibling concerns. Put focused algorithms and lifecycle work in
  owned submodules.
- Choose a module boundary from responsibility and runtime ownership, not line count alone. Split
  when code has different state owners, execution roles, feature gates, schedule phases, reasons to
  change, or independently testable algorithms. Do not create one plugin, architectural layer, or
  file per type merely to make files shorter.
- A schedule-facing Bevy system should coordinate a recognizable phase. When it grows to combine
  validation, candidate collection, deterministic ordering, mutation, telemetry, and cue emission,
  extract named helpers or focused systems while keeping ordering explicit. Moving one giant
  function unchanged into another file is not decomposition.
- Preserve fixed-tick ordering and deferred-command boundaries during extraction. Keep meaningful
  `SystemSet`, `.before`/`.after`, `.chain()`, physics refresh, and `ApplyDeferred` relationships
  visible at the composition point; add schedule tests when changing them.
- Keep execution roles strict. Authoritative mutation belongs to server-gated combat, movement, or
  session modules. Client modules sample intent and present replicated state/cues. Process evidence
  and verification may observe gameplay but must not become a second gameplay or mutation path.
- Keep authored definitions, selected/resolved builds, mutable ECS runtime state, protocol
  registration, telemetry/evidence, and presentation as separate concerns. A shared wire shape does
  not authorize shared execution of server-only rules.
- Treat data-only extension and semantic-family extension differently. New instances composed from
  existing typed mechanics belong in authored catalogs. A new behavior family adds a bounded typed
  schema or runtime capability plus its focused plugin/handler/system; do not add built-in-ID
  branches to unrelated coordinators.
- Registration builders exist only during plugin composition. They use stable IDs, enforce capacity,
  duplicate and required-coverage rules, sort deterministically, then seal immutable resources.
  Process-local registries do not open stable wire enums or create runtime-loaded executable mods.
- Every new shared headless gameplay catalog registers canonical fingerprint material through its
  content plugin. Client audio/VFX and other presentation catalogs remain outside the gameplay
  fingerprint and the server feature graph.
- Model authoritative cross-domain work as collect/validate/plan/commit/publish when atomicity or
  ordering matters. Map owns object selection and topology; combat alone commits fighter/deployable
  health, lifecycle, outcomes, and cues for environmental damage.
- Presentation feature producers emit stable-key `VfxRequest` or `AudioRequest` messages. Generic
  client adapters own authored-profile lookup and Bevy materialization; producers do not select
  renderer/material families, asset handles, playback policy, or reduced-effects behavior.
- AI consumes semantic mode, objective, world-object, and effect-tile projections rather than
  recognizing concrete asset IDs or inspecting mode-specific resources. Mode installation goes
  through `src/modes/` registrations while the routed `GameMode` protocol remains closed.
- Effect tiles currently resolve to focused runtime capabilities; there is no generic executable
  handler registry. Add one only after a second real mechanic demonstrates a reusable handler shape.
- Keep network registrations in `protocol.rs`; keep stable shared protocol/gameplay types in the
  appropriate shared model/cue/definition module. Never expose process-local `Entity` identity on
  the wire. Preserve public module paths and wire contracts during organization-only changes unless
  the active ticket explicitly approves and tests a protocol change.
- Follow `docs/08-network-architecture.md` for application protocol evolution: use the one global
  compatibility handshake and current schema, and do not introduce per-message versions or
  compatibility decoders without a new validated architecture decision.
- Default new items and submodules to private. Use `pub(crate)` for demonstrated cross-module use
  and public re-exports only for the crate API consumed by another role, integration tests, or a
  genuine external boundary. Avoid wildcard re-exports that accidentally turn implementation
  details into API.
- Feature-gate role-owned modules at their ownership boundary. The server feature graph must not
  acquire windowing, rendering, audio, device input, or client assets through a convenient shared
  module. Run role-specific checks after moving imports or types across client/server boundaries.
- Avoid module/file-wide complexity suppressions. A necessary Clippy exception for a Bevy system
  query or deterministic orchestration function should be attached narrowly to that item and remain
  reviewable. New `too_many_lines`/`too_many_arguments` findings are prompts to inspect ownership
  and decomposition before adding an allow.
- Place pure rule tests beside the owning module, using `tests.rs` when a focused module's tests
  would otherwise obscure production code. Put separate-App authority/replication behavior under
  `tests/network/`, reuse `harness.rs`, and group scenarios by behavior rather than accumulating
  them in `tests/network.rs` or duplicating harness setup.
- When a file is already large but cohesive, add new code only if it shares that exact ownership and
  lifecycle. A new concern should get a named submodule; recurring growth inside one system should
  be decomposed into testable helpers. Do not use a hard line limit as a substitute for this review.

## Value, maintainability, and no-over-engineering rules

- Deliver a complete player-visible vertical slice before building general infrastructure. A
  feature ticket should end with functional value a player can exercise, not only reusable
  machinery.
- Build for current demonstrated requirements. Do not model future screens, states, protocol
  variants, settings migrations, widget variants, or extension points before an owned use exists.
- Start with local, direct code. Extract a helper, module, plugin, crate, or public API only after
  duplication, distinct ownership, platform separation, testing needs, or another concrete cost
  demonstrates the boundary. A second real use is evidence; an imagined future use is not.
- Prefer Bevy-native components, resources, systems, states, messages, assets, and UI before
  adding a custom framework or dependency. Add another abstraction only when the native approach
  creates a specific observed problem.
- Optimize for obvious ownership and readable execution flow, not the number of layers. A small
  action enum and coordinating system are preferable to reducers, command buses, callbacks, or
  multi-stage state machines when the feature does not require those mechanisms.
- Preserve the boundaries that protect the product: server authority, execution-role isolation,
  stable wire identity, recoverable persistence, bounded state, and accepted automation paths.
  Avoid generalizing behavior outside those boundaries without evidence.
- Keep presentation optional around behavior. Animation, audio, effects, and transitions must not
  become the authority for navigation, saving, networking, shutdown, or gameplay state.
- Organize by responsibility and lifecycle rather than line count. A cohesive file may remain
  moderately large; split it when responsibilities or owners diverge and the resulting boundary is
  easier to understand and verify.
- Test costly risks and important contracts, not every combination. Use focused pure/ECS tests,
  representative integration cases, and a small visual/manual matrix. Do not multiply every state,
  input, resolution, scale, timing sample, and failure into a Cartesian suite without evidence.
- Reuse production components, canonical commands, and existing harnesses. Do not create a general
  abstraction solely to make one test possible unless production code also benefits from the seam.
- Record deferred polish and known limitations in the owning ticket or create a separate backlog
  ticket. Do not expand the current slice incidentally to solve future work.
- Prefer the smallest clear implementation that owns today's behavior and is easy to change when a
  new requirement becomes real. Maintainability means clear ownership, limited scope, and safe
  change—not maximum abstraction.

## Implementation and verification rules

- The active ticket description and spec are the implementation scope contract. Update them through
  the Ticket CLI and revalidate acceptance criteria before materially changing scope or
  architecture.
- Server authority is not optional, including in-process and offline development modes.
- Clients send intent, not positions, hits, damage, scores, status triggers, or map edits.
- Separate authored definitions, selected builds, and runtime state.
- Keep gameplay facts/messages independent from rendering, audio, camera, and HUD presentation.
- Put player-facing balance in authored catalogs or validated operator configuration. Keep
  representation, protocol, capacity, overflow, and deterministic-geometry ceilings code-owned.
  Shared passive/weapon/ultimate bounds live beside their typed model and are consumed by both the
  authoritative validator and private Balance Lab adapters.
- Use `src/timing.rs` conversions rather than literal 60-Hz math. Resolve formation deadlines from
  `config/server/game-types.ron` and direct-diagnostic loadouts from
  `content/catalogs/builds.ron`; do not infer either from catalog order or literals.
- Use focused pure-function tests where a rule is naturally independent of ECS. Test component, resource, lifecycle, and state behavior with small `App`/`World` schedule tests; add headless integration tests for authority and replication.
- Advance Bevy fixed time or explicitly run the relevant schedule in time-dependent tests rather than waiting on wall-clock sleeps.
- Visual verification complements automated tests; it does not replace them.
- Preserve unrelated user changes and keep deferred work visible in separate backlog tickets.

Canonical build, test, process, closeout, and playtest commands already live in `justfile` and the
root `README.md`; use those rather than inventing substitutes. The completed V3 renderer has no 2D
gameplay-world fallback or user-facing renderer choice. `BRAWLER_FORCE_PRIMITIVE_WORLD` selects
deterministic meshes inside the same 3D composition and must not become a permanent content mode.


## Local implementation references

Use the checked-in source and examples before guessing an API or copying an unrelated internet snippet, but verify snapshot versions before transferring exact APIs:

- `references/bevy/examples/` — official Bevy example source. Start with `README.md`, then locate focused examples with `rg`; useful foundation examples include `app/headless.rs`, `app/plugin.rs`, and `app/plugin_group.rs`.
- `references/lightyear/examples/` — official Lightyear example projects and their `Cargo.toml` feature sets. Start with `README.md`; use `simple_setup` for minimal client/server composition, `simple_box` for authoritative replication/prediction/interpolation, and `avian_2d` only when physics integration is in scope.
- `references/lightyear/book/` — local Lightyear book. Start with `src/SUMMARY.md`, then read the relevant tutorial or concept pages for protocol, transport, replication, inputs, system ordering, shared plugins, client/server setup, prediction, interpolation, and Avian integration.
- `references/avian/crates/avian2d/examples` — official Avian 2D examples project and their `Cargo.toml` feature sets.

The Lightyear 0.29 snapshot targets Bevy 0.19, while the checked-in Bevy source is currently 0.20-dev. Use the Bevy snapshot for architectural examples, but confirm exact APIs against Bevy 0.19 source or official documentation before implementation.

Treat `references/` as read-only upstream material unless the user explicitly requests a snapshot update. Inspect the example README, source, and `Cargo.toml` together because feature flags and application topology are part of the example. Adapt the smallest relevant pattern to Brawler's authority and dependency boundaries; do not copy whole examples blindly.

When research still requires the internet, prefer current primary documentation and record why the local snapshot was insufficient.

## Ticket-driven work tracking

Ticket is the source of truth for active planning and progress. Do not create a new big numbered
version, version roadmap, or milestone file to track current work. The completed V1–V12 material
under `docs/implementation/` remains historical evidence for prior decisions, implementation,
verification, feedback, and learning. Durable product or technical behavior still belongs in the
owning main documentation rather than being defined only in a ticket.

This repository's Ticket project code is `BRL`; create and update Brawler work in that project.
Ticket itself is a new application and may still contain defects or workflow rough edges. When use
of the CLI reveals a Ticket bug or a concrete way the tool could work better, create a ticket for
the Ticket application with `ticket task new --project TCK <title>` rather than adding it to the
`BRL` backlog.

Use the repository's `ticket` CLI and its installed skill for every new task:

- Search or list existing tickets before creating one, and continue the existing ticket when its
  scope matches rather than creating a duplicate.
- Give each independently reviewable task one ticket. Its description states the outcome; its spec
  owns scope, decisions, acceptance criteria, implementation constraints, verification, playtest
  needs, feedback disposition, and closeout learning.
- Use the actual Ticket statuses: `idea`, `backlog`, `todo`, `doing`, `done`, and `canceled`. Move a
  ticket to `doing` when implementation starts and to `done` only when its acceptance criteria and
  required evidence are satisfied.
- Record material scope or architecture changes in the ticket spec before implementing them. If a
  required choice exceeds the user's existing direction, add a Ticket question and wait for its
  answer; resolve answered questions after incorporating the decision.
- Create another linked or clearly referenced backlog ticket for deferred work instead of silently
  expanding the active ticket.
- Use `ticket task <ID> desc|spec|comment|question|link` commands for mutations. Never edit
  `.ticket/` or `docs/tasks/` directly. Those files are Ticket-owned state and readable mirrors.
- Run `ticket sync` before handoff. If it reports a conflict, stop and surface the exact conflict;
  never choose a side on behalf of the user.

## Ticket task process

For active work:

1. Inspect `ticket list`, `ticket search`, and any referenced ticket. Create a ticket only when no
   existing ticket owns the request.
2. Set a concise outcome-oriented description and a concrete spec with acceptance criteria,
   relevant constraints, and proportional verification. Move it to `doing` when work begins.
3. Inspect the relevant local source, documentation, and Bevy/Lightyear references before guessing.
   Add research findings and any consequential decision to the ticket spec.
4. Implement the ticket without silently broadening its scope. Keep authoritative ownership,
   execution-role isolation, stable wire identity, persistence, and bounded-state contracts intact.
5. Run focused and canonical verification proportional to risk. Record commands, results, and any
   intentionally manual or unavailable evidence in the ticket.
6. When native or subjective playtesting is required, provide the run path, controls, scenario,
   known limitations, and requested observations. Keep the ticket in `doing` while required
   feedback or corrections remain.
7. Record every feedback item as implemented, deferred to another ticket, rejected with rationale,
   or awaiting evidence. Re-run affected verification after accepted corrections.
8. Perform a learn-from-errors review for substantial work. Record mistakes, causes, prevention,
   and reusable lessons in the ticket; create or improve a skill only when the learning genuinely
   recurs.
9. Move the ticket to `done` only when its acceptance criteria, verification, feedback disposition,
   durable documentation, and required learning are complete. Run `ticket sync` and report the
   ticket ID in the handoff.

<!-- REPOWISE_AGENTS:START — Do not edit below this line. Auto-generated by Repowise. -->
## Codebase Intelligence for brawler (Repowise)

Indexed by [Repowise](https://repowise.dev). Last indexed: 2026-08-29 (commit 535d3ec). Confidence: 100%.
### How to work in this repo

- **Trust the index.** `verified: true` means the bytes were checked against the live tree, so never re-read those lines. Re-read only on `bounds: "approximate"`, `_meta.stale_warning`, `search_method: "bm25"` or `confidence: "low"`; `index_behind: true` alone is informational.
- **Pre-edit, not instead-of-edit.** These tools decide *which* files to read and edit. Reading a file before you edit it is correct and expected.
- **Noisy commands** (tests, builds, `git log`/`diff`, searches, listings): prefer `repowise distill <cmd>`, the same command with its exit code preserved and errors-first output. A `[repowise#<ref>: N lines omitted]` marker is recoverable via `repowise expand <ref>` (add `-q <regex>` to filter); never re-run the command to see omitted output.
- **Recording a decision** you had to reason out: `repowise decision add --title T --decision D` records it without prompting and prints the id (`--format json` to parse it back). It lands `proposed`, for a person to confirm.

### Tools

| Tool | When and why |
|------|--------------|
| `get_answer(question)` | First call for any how/where/why question. Cite `confidence: "high"` or `grounding: "extracted"` directly; `degraded` means judge by `retrieval_quality`. `symbol_bodies` has live bodies. |
| `get_context(targets=[...])` | Triage card for files/modules/symbols: docs, signatures, hotspot, fix history. No source bytes — `include=["skeleton"]` for the whole file verified, `["callers"|"decisions"]` for depth. Batch targets. |
| `get_symbol(id, depth?)` | **Follow-up, not an entry point** — one verified body for an id a prior response named (`path.py::Name`, `path.py:140-180`, `repowise#<hex>`). Never walk a file symbol by symbol; Read it. |
| `search_codebase(query)` | Hybrid search, auto-routed by query shape; force with `mode=symbol|path|concept|hybrid`. A hit whose `sources` are `[fts]` only has no semantic agreement, so verify it. |
| `get_why(query, targets?)` | Why the code is shaped this way: decision records, git archaeology, rationale comments. Call before a refactor or a pattern divergence. |
| `get_risk(targets, changed_files?, include?)` | File history and structural reach. PR mode leads with `directive`; its 0-10 structural heuristic is uncalibrated, not a probability. Read typed test recommendations and coverage state first. |
| `get_change_risk(revspec?, extensions?, exclude_patterns?)` | Deterministic live-diff review signal for a commit or range. Lead with benchmarked percentile/classification; the 0-10 diff-shape score is supporting, not a probability. `get_risk` scores paths. |
| `get_health(targets?, include?)` | Defect / maintainability / performance scores and findings. Self-check the files you touched before finishing. |
| `get_dead_code(tier?, min_confidence?, safe_only?)` | Confidence-tiered unreachable files / unused exports / zombie packages. For cleanup sweeps, not targeted fixes. |
| `get_overview()` | Architecture map. Call once, first, in an unfamiliar repo; skip it after that. |

### Architecture
**Files:** 448 | **Lines:** 204293 | **Monorepo:** 2 packages | **Import cycles:** 3
brawler is a multi-package rust codebase of 448 files, split across 2 packages. Execution starts at src/bin/server.rs, src/combat/server.rs, src/map/server.rs and 5 other entry points. Start here when reading the codebase. Ranked by PageRank over the import graph: the files most of the codebase ultimately depends on.

### Key modules
- `src/client/presentation_3d` — src/client/presentation_3d · src/client/presentation_3d/combat
**Language:** rust | **Files:** 14 | **Public symbols:** 0 / 641
Covers the…
- `packages/brawler-routing/src` — packages/brawler-routing/src
**Language:** rust | **Files:** 17 | **Public symbols:** 520 / 1124
Covers the 17 source files in…
- `src/combat` — src/combat
**Language:** rust | **Files:** 11 | **Public symbols:** 392 / 837
Covers the 11 source files in src/combat
- `tools/balance-lab-web/src/pages/balance-lab` — tools/balance-lab-web/src/pages/balance-lab · tools/balance-lab-web/src/pages/balance-lab/api ·…
- `src/client` — src/client · src/client/flow · src/client/flow/screens · src/client/presentation_3d · src/client/presentation_3d/combat ·…
- `src` — src
**Language:** rust | **Files:** 9 | **Public symbols:** 255 / 484
Covers the 9 source files in src
- `src/map` — src/map · src/profiles
**Language:** rust | **Files:** 13 | **Public symbols:** 583 / 1106
Covers the 13 source files in 2 directories…
- `src/matchplay` — src/matchplay · src/movement
**Language:** rust | **Files:** 13 | **Public symbols:** 327 / 640
Covers the 13 source files in 2 directories…
- `src/combat/client` — src/combat/client · src/combat/definitions · src/combat/effects
**Language:** rust | **Files:** 10 | **Public symbols:** 129 / 379
Covers…
- `src/bots` — src/bots · src/builds
**Language:** rust | **Files:** 12 | **Public symbols:** 222 / 472
Covers the 12 source files in 2 directories under…

### Entry points
- `src/matchplay/server.rs`
- `src/map/server.rs`
- `src/combat/server.rs`
- `src/bin/server.rs`
- `tools/balance-lab-web/src/main.tsx`
- `tools/balance-lab-web/src/app/App.tsx`

### Files that need care (bug-fix history first, then churn — check `get_risk` before editing)
- `src/client/tests.rs` — 7 bug fixes, last fix 10 days ago (bug magnet); 27 commits/90d
- `src/client/mod.rs` — 6 bug fixes, last fix 10 days ago (bug magnet); 35 commits/90d
- `src/client/input.rs` — 6 bug fixes, last fix 10 days ago (bug magnet); 18 commits/90d
- `scripts/network.sh` — 5 bug fixes, last fix 11 days ago (bug magnet); 24 commits/90d
- `src/diagnostics/tests.rs` — 4 bug fixes, last fix 11 days ago (bug magnet); 12 commits/90d

### Code health
Three co-equal signals: defect risk 5.72/10 avg, hotspot health 4.16/10 (stable), worst `src/abilities/dash.rs` at 1.0/10 · maintainability 6.95/10 · performance risk 70 open static I/O-in-loop / N+1 findings. Detail: `get_health()`.

Critical files:
- `src/client/assets.rs` — complex conditional (validate_manifest) — impact −2.5
- `docs/implementation/v12/milestone-01.md` — change entropy — impact −2.3
- `src/combat/mod.rs` — complex conditional (validate) — impact −2.0
- `src/weapon_parts/resolver.rs` — untested hotspot — impact −2.0
- `src/server/verification.rs` — untested hotspot — impact −2.0

<!-- REPOWISE_AGENTS:END -->
