# Brawler repository guide

## Quick orientation

Brawler is an original, cross-platform top-down arena shooter built around player-authored fighter builds. Combat readability, meaningful build tradeoffs, short matches, reusable content primitives, and server-authoritative networking are the core product constraints.

For current work, start with `ticket list` and `ticket task <ID>`. Ticket descriptions and specs
own active scope, status, acceptance criteria, and evidence. `docs/tasks/` contains readable Ticket
mirrors but is never the agent write path.

Start with:

1. `docs/00-product-direction.md` for product intent and non-goals.
2. `docs/implementation/v12/roadmap.md`, `milestone-03.md`, `docs/03-weapons-and-abilities.md`, and
   `docs/15-balance-lab.md` for the completed 3v3 maps and framing, Balance Lab correction,
   server-authoritative sustain/ammunition recovery, evidence capture, projectile readability, and
   feedback-driven balance closeout.
3. `docs/implementation/v11/roadmap.md`, `milestone-01.md`, and `docs/10-bots.md` for the completed
   playable server-hosted Practice bots, fair delayed perception, bounded navigation, objective
   behavior, feedback corrections, and V11 closeout.
4. `docs/implementation/v10/roadmap.md`, `milestone-03.md`, and
   `docs/18-damageable-world-objects-and-heist.md` for the completed oil barrels, mirrored Heist,
   consolidated Feature Yard family, treasure chests, restoration pickups, and V10 closeout.
5. `docs/implementation/v9/roadmap.md`, `milestone-03.md`, and `docs/17-concealment.md` for the
   completed authoritative terrain, Self Cloak, Concealment Field, reveal-proximity, Reveal Scan,
   server-advertised brawler catalog, and saved-brawler UI closeout.
6. `docs/implementation/v8/roadmap.md`, `milestone-04.md`, and
   `docs/16-grid-map-asset-system.md` for the completed sparse map-asset system, domain-organized
   content, and legacy-removal evidence.
7. `docs/implementation/v7/roadmap.md` for the completed persistent server-owned profiles, saved
   brawlers, weapon bases, and four-slot weapon-part equipment.
8. `docs/implementation/v6/roadmap.md` and `docs/15-balance-lab.md` for the completed
   development-only authoritative Balance Lab and its enduring operator contract.
9. `docs/implementation/v5/roadmap.md` and `milestone-03.md` for the completed auto-connect,
   responsive Player Dashboard, connected-loop convergence, recovery/lifecycle hardening, and V5
   closeout evidence.
10. `docs/implementation/v4/roadmap.md` and `milestone-03.md` for the historical independently
   embedded map documents, semantic object placement, two reusable themes, routed admission,
   presentation hardening, and V4 closeout evidence.
11. `docs/implementation/v3/roadmap.md` for the completed 3D-presentation migration and enduring
   V3 decisions.
12. `docs/11-art-and-presentation-direction.md` for the current renderer, readability, asset,
   provenance, degradation, and future-art contracts.
13. `docs/08-network-architecture.md` for enduring gameplay authority and replication boundaries.
14. `docs/13-player-ux.md` for the canonical player experience and
   `docs/14-multiplayer-server-architecture.md` for the routed-process decisions it relies on.
15. `docs/implementation/v2/roadmap.md` and `milestone-09.md` for the completed routed product
   baseline and closeout evidence.
16. `docs/implementation/v1/roadmap.md` and `milestone-11.md` for the completed gameplay MVP,
   verification evidence, deferred release polish, and the direct-UDP comparison baseline.

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
  lib.rs                   shared crate module and role feature gates
  bin/{client,server}.rs   thin executable entry points
  gameplay.rs              shared fixed-tick schedule/set composition
  protocol.rs              wire registration and network protocol boundary
  config.rs                validated client/server/process configuration
  content.rs               build-embedded catalog loading and content fingerprints
  timing.rs                shared simulation time definitions
  abilities/
    mod.rs                 ability composition root, schedule sets, public re-exports
    charge.rs              ultimate charge ownership and outcome observation
    dash.rs                authoritative dash activation/movement/interruption
    sentry.rs              deployable activation, targeting, firing, and cleanup
    passives.rs            passive trigger/application rules
    telemetry.rs           bounded ability records and aggregates
    tests.rs               focused ability composition and behavior tests
  builds/
    mod.rs                 build composition root and public API
    model.rs               authored selections and resolved immutable loadouts
    definitions.rs         catalogs, validation, resolution, fingerprints
    server.rs              waiting-phase authoritative build transaction
    telemetry.rs           bounded selection/build records and aggregates
    tests.rs               focused build rule and composition tests
    combat/
    mod.rs                 combat composition root, public re-exports, shared sets/plugins
    model.rs               stable identities and shared/runtime combat state shapes
    cues.rs                gameplay-to-presentation combat facts
    definitions/           authored catalog, validation, resolution, fingerprints, tests
    authority.rs           authoritative fighter lifecycle and authority helpers
    attack.rs              economy, attack acceptance, firing expansion, attack telemetry
    delivery.rs            straight, lobbed, and melee delivery geometry/execution
    effects/               staged payload planning/application/runtime transaction and tests
    outcomes.rs            bounded authoritative outcome-fact ownership
    telemetry.rs           bounded records, trackers, aggregates, summaries
    evidence.rs            bounded process/checkpoint evidence and convergence schemas
    server.rs              server combat plugin and schedule registration
    client/                previews, cues, transient observations, HUD, and tests
    tests.rs               shared combat model/composition tests
  map/
    mod.rs                 map composition root, stable IDs/profiles, public re-exports
    model.rs               stable map identity, bounds, spawn, and shared placement types
    catalog.rs             map-asset catalogs, recipes, validation, resolution, and tests
    objects.rs             damageable-target identities, health-state shapes, facts, and cues
    pickups.rs             restoration-pickup authority, collection, expiry, reset, and telemetry
    runtime/               server map schedule composition, object authority, installation/colliders,
                           dynamic destruction/recovery, and focused tests
    server.rs              selected-map startup and exact-generation lifecycle
    client.rs              replicated map convergence, recovery, and presentation readiness
  matchplay/
    mod.rs                 common match schedule/restart composition and mode plugins
    model.rs               stable match state, results, participants, and summaries
    lifecycle.rs           fighter defeat/respawn/reset lifecycle helpers
    server.rs              common authoritative roster, phase, restart, and outcomes
    spawns.rs              mode-neutral team assignment and deterministic spawn selection
    wipeout.rs             Wipeout scoring and mode-owned reset
    hot_zone.rs            Hot Zone occupancy/progress and mode-owned reset
    heist.rs               mirrored objective health, threshold/timeout outcomes, and mode reset
    telemetry.rs           bounded match/mode records and aggregates
    tests.rs               focused common/mode lifecycle tests
  movement/
    mod.rs                 movement plugins and authoritative schedule composition
    arena.rs               arena definitions, geometry, colliders, and spawn helpers
    authority.rs           server-owned movement decisions, collision, and mutation
    input.rs               pure input shaping plus server validation/freshness rules
    tests.rs               focused movement tests
  client/
    mod.rs                 client application composition and shared client state
    assets.rs              retained visual/audio handles and readiness
    audio.rs               bounded cue-to-audio presentation
    hud.rs                 session, combat, build, match, and mode HUD
    input.rs               keyboard, mouse, gamepad, and native-input sampling
    presentation.rs        screen-space HUD and pause-overlay UI
    presentation_3d/       sole gameplay-world renderer: camera, coordinates, GLB/animation,
                           map/combat meshes, projected fighter UI, and render diagnostics
    session/               connection/admission, routed transitions, match commands, automation,
                           observation, shutdown, and explicit session schedule composition
    settings/              local calibration/rebinding state and pause-overlay UI
    tests.rs               client composition and behavior tests
  diagnostics/
    mod.rs                 closeout schemas, aggregation, registration, and public API
    failure.rs             bounded process failure classification
    overlay.rs             client authority/network diagnostics presentation
    process.rs             process-owned report/checkpoint lifecycle
    tests.rs               schema, lifecycle, and validation tests
  server/
    mod.rs                 dedicated-server and connection/session composition
    verification.rs        process-only movement/combat evidence validation
    tests.rs               server composition and lifecycle tests
tests/
  network.rs               integration-test composition entry point
  network/
    harness.rs             reusable separate-App/Crossbeam/UDP test harness
    *.rs                   scenarios grouped by lifecycle, movement, map, selection, builds, combat, recovery, and modes
  performance.rs           fixed-tick and subsystem performance/capacity gates
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
- Prefer Bevy-native components, resources, systems, states, events/messages, assets, and UI before
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
- Keep gameplay events independent from rendering, audio, camera, and HUD presentation.
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

## Codebase Intelligence for brawler (Repowise)

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

