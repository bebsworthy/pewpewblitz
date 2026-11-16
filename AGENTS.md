# Brawler repository guide

PewPew Blitz (repository and crate identifier `brawler`) is an original, cross-platform top-down
arena shooter built around player-authored fighter builds. Product constraints: readable combat,
meaningful build tradeoffs, short matches, reusable content primitives, and server-authoritative
networking.

Current work lives in Ticket (project `BRL`), never in version or milestone files. Start with
`ticket list` and `ticket task <ID>`; see [Ticket workflow](#ticket-workflow).

## Where to look

| Topic | Documents |
|---|---|
| Doc map, product intent, non-goals | `docs/README.md`, `docs/00-product-direction.md` |
| Bevy/ECS, authority, schedules, protocol compatibility, registry and content fingerprints | `docs/01-engine-decision.md`, `docs/08-network-architecture.md` |
| Fighter definitions, resolved loadouts, combat transactions, extension rules | `docs/02-fighter-model.md`, `docs/03-weapons-and-abilities.md` |
| Maps, effect tiles, modes, world objects | `docs/04-maps-and-game-modes.md`, `docs/16-grid-map-asset-system.md`, `docs/18-damageable-world-objects-and-heist.md` |
| Practice bots, perception, concealment | `docs/10-bots.md`, `docs/17-concealment.md` |
| Renderer, audio/VFX boundaries, assets, degradation | `docs/11-art-and-presentation-direction.md` |
| Player flow, routed processes, Balance Lab | `docs/13-player-ux.md`, `docs/14-multiplayer-server-architecture.md`, `docs/15-balance-lab.md` |
| History (V1–V12, completed 2026-08-18 to 2026-08-27) | `docs/implementation/v*/` — evidence for past decisions only |

V1–V12 are closed historical records. Later draft version documents are context, not active scope.
Durable product or technical behavior belongs in the owning `docs/` document, not only in a ticket.

## Stack

- Bevy 0.19.1 (ECS, plugins, 3D world, UI, input, assets, animation, audio), Lightyear 0.29.0
  (transport, input, replication, interpolation, owner prediction), Avian 2D 0.7.0 (authoritative
  planar collision and map colliders; no 3D physics).
- Fixed-tick, dedicated-server-authoritative simulation. macOS is the primary dev target; Android
  (Pixel) is the mobile target.
- Bevy's `World` is the gameplay model. Prefer Bevy-native patterns, the local references, and
  verified Bevy 0.19 APIs over general Rust architecture advice. DDD/hexagonal is not the governing
  model; use ports/adapters only at a concrete external boundary with an observed problem.

## Source layout

Code is organized by ECS state ownership, execution role, plugin composition, and schedule phase.
Replicated types stay in `brawler` (the protocol registry fingerprint hashes their paths), and
`brawler` never depends on `brawler-client`.

```text
src/                       shared `brawler` crate: gameplay, protocol, headless server config
  lib.rs gameplay.rs protocol.rs config.rs content.rs timing.rs
  bin/{server,profile_admin}.rs
  abilities/ builds/ combat/ concealment/ movement/ weapon_parts/ profiles/
  map/  map/runtime/       catalog/resolution/effect tiles; installation, dynamics, object authority
  modes/                   bounded local registry: Wipeout, Hot Zone, Heist (+ Zombie)
  matchplay/               lifecycle, spawns, per-mode rule plugins
  bots/                    private Practice policy, observations, navigation
  server/                  admission, fighter spawn, lobby, balance_lab, practice, workers
  diagnostics/             process closeout, sampling, failure records
packages/brawler-client/   player client (Bevy app; `client` feature boundary)
  src/{flow,queue,session,audio,vfx,presentation_3d,settings,touch,ui,hud,…}
  src/bin/{client,vfx_lab,ui_gallery}.rs
packages/brawler-routing/  route envelope, IPC, supervisor/worker protocol
packages/brawler-android/  thin Android GameActivity shell
content/{catalogs,maps}/   headless-safe authored gameplay data (in the gameplay fingerprint)
assets/catalogs/           client-only audio/VFX/map presentation data (outside the fingerprint)
config/server/             game types, formation deadlines
tools/balance-lab-web/     TypeScript operator app
tests/network/             separate-App authority/replication scenarios + `harness.rs`
                           (a `brawler-client` test target, feature `network-test`)
tests/performance.rs       fixed-tick and capacity gates
references/                read-only upstream snapshots (Bevy, Lightyear, Avian)
```

Schema versions live in documents and Rust compatibility constants, not directory names.

## Commands

The `justfile` and `README.md` own canonical commands; don't invent substitutes.

- `just lint` — fmt, Clippy for every role, server/client feature isolation, layout checks. Needs
  `ripgrep` on `PATH` for the shell scripts.
- `just test` — all deterministic suites, including network and performance gates. Focused network
  tests: `cargo test -p brawler-client --features network-test --test network <filter>`.
- `just practice-e2e`, `just rejoin-e2e`, `just network-change-e2e`, `just e2e N` — real-process
  routed checks.
- `just server` — routed supervisor (default `127.0.0.1:5000`, data in `target/dev/server`). Set
  `BRAWLER_ROUTED_BIND=0.0.0.0:5000` so a phone on the LAN can reach it.
- `just client`, `just run N` — native clients against the routed topology.
- `just android-build <host:port>` / `just android-install` — release APK with the server address
  prefilled.
- Scripts that start `brawler-supervisor` without `--data-directory` write the local,
  gitignored `data/server/`; point new automation at a `target/` directory.
- `scripts/network.sh` is only the legacy direct-UDP diagnostic baseline.
  `BRAWLER_FORCE_PRIMITIVE_WORLD` swaps in primitive meshes inside the single 3D renderer; it is
  not a renderer choice or content mode.

## Invariants

- Server authority holds in every mode, including in-process and offline development. Clients send
  intent, never positions, hits, damage, scores, status triggers, or map edits.
- The server feature graph never pulls in windowing, rendering, audio, device input, or client
  assets. Feature-gate role-owned modules at their ownership boundary and run the role-specific
  checks after moving types across it.
- Authoritative mutation lives in server-gated combat, movement, or session modules. Client modules
  sample intent and present replicated state and cues. Evidence and verification code may observe
  gameplay but never mutate it. Share a system between roles only when it truly runs in both, such
  as measured owner prediction.
- Keep authored definitions, selected/resolved builds, runtime ECS state, protocol registration,
  telemetry/evidence, and presentation separate. A shared wire shape doesn't authorize shared
  execution of server-only rules.
- Network registrations live in `protocol.rs`. Never put a process-local `Entity` on the wire; use
  stable player, match, and definition IDs. Organization-only changes preserve public module paths
  and wire contracts. Protocol evolution follows `docs/08-network-architecture.md`: one global
  compatibility handshake, no per-message versions or compatibility decoders.
- Use `src/timing.rs` conversions, never literal 60 Hz math. Formation deadlines come from
  `config/server/game-types.ron` and diagnostic loadouts from `content/catalogs/builds.ron`, never
  from catalog order or literals.
- Player-facing balance goes in authored catalogs or validated operator config. Representation,
  protocol, capacity, overflow, and geometry ceilings stay in code. Shared passive, weapon, and
  ultimate bounds live beside their typed model and serve both the authoritative validator and the
  Balance Lab adapters.
- Every new headless gameplay catalog registers canonical fingerprint material through its content
  plugin.
- Registration builders exist only during plugin composition: stable IDs, capacity, duplicate and
  coverage rules, deterministic sort, then seal into immutable resources. They never open stable
  wire enums or load executable mods at runtime.
- Cross-domain authoritative work is collect, validate, plan, commit, publish when atomicity or
  ordering matters. Map owns object selection and topology; combat alone commits fighter and
  deployable health, lifecycle, outcomes, and cues for environmental damage.
- Presentation producers emit stable-key `VfxRequest` / `AudioRequest` messages. Generic client
  adapters own profile lookup and Bevy materialization. Presentation never becomes the authority
  for navigation, saving, networking, shutdown, or gameplay state.
- AI consumes semantic mode, objective, world-object, and effect-tile projections, never concrete
  asset IDs or mode-specific resources. Modes install through `src/modes/`; the routed `GameMode`
  protocol stays closed. Effect tiles resolve to focused runtime capabilities. Add a generic handler
  registry only after a second real mechanic shows the shape.

## Code organization

- Split modules by responsibility, state owner, execution role, feature gate, schedule phase, or
  independently testable algorithm, not by line count. Don't create a plugin, layer, or file per
  type. A large but cohesive file may grow only with code of the same ownership and lifecycle.
- `mod.rs` is composition and intentional API: system sets, shared resources, plugin installation,
  small re-exports. Algorithms and lifecycle work go in owned submodules.
- A schedule-facing system coordinates one recognizable phase. When it mixes validation, collection,
  ordering, mutation, telemetry, and cue emission, extract named helpers or systems. Moving a giant
  function unchanged into another file is not decomposition.
- Keep `SystemSet`, `.before`/`.after`, `.chain()`, physics refresh, and `ApplyDeferred` ordering
  visible at the composition point. Add schedule tests when changing it.
- New behavior: a new instance of an existing typed mechanic goes in an authored catalog. A new
  behavior family adds a bounded typed schema or runtime capability and its own focused
  plugin/system. Never add built-in-ID branches to unrelated coordinators.
- Default to private. Use `pub(crate)` for demonstrated cross-module use and `pub` only for the API
  another role, integration test, or external boundary consumes. No wildcard re-exports.
- No module- or file-wide lint suppressions. Attach a necessary Clippy allow narrowly to the item
  with a reason. A new `too_many_lines` / `too_many_arguments` finding means reconsider ownership
  first.

## Scope and simplicity

- Deliver a complete player-visible slice before general infrastructure. Build for current
  demonstrated requirements: no speculative screens, states, protocol variants, settings
  migrations, widget variants, or extension points.
- Start local and direct. Extract a helper, module, plugin, crate, or public API only when a second
  real use, distinct ownership, platform separation, or a testing need demonstrates the boundary.
- Prefer Bevy-native components, resources, systems, states, messages, assets, and UI over custom
  frameworks or new dependencies. A small action enum plus a coordinating system beats reducers,
  command buses, or multi-stage state machines the feature doesn't need.
- Record deferred polish and known limitations in the owning ticket or a new backlog ticket. Don't
  expand the current slice to absorb them.

## Testing

- Pure rules: unit tests beside the owning module (`tests.rs` when inline tests would obscure the
  code). Components, resources, lifecycle, and schedules: small `App`/`World` tests. Authority and
  replication: `tests/network/`, reusing `harness.rs` and grouped by behavior.
- Advance Bevy fixed time or run the schedule explicitly; never wait on wall-clock sleeps.
- Test costly risks and important contracts, not Cartesian products of every state, input,
  resolution, and timing. Reuse production components and existing harnesses; add a seam only if
  production code also benefits.
- Visual and on-device checks complement automated tests; they don't replace them.

## Local references

`references/` is read-only upstream material; update it only on explicit request. Check it before
guessing an API, and read the example's `README.md`, source, and `Cargo.toml` together, since
feature flags and app topology matter. Adapt the smallest relevant pattern; don't copy whole
examples.

- `references/bevy/examples/` — Bevy examples. The snapshot is **0.20-dev**; confirm exact APIs
  against Bevy 0.19 source or docs.
- `references/lightyear/examples/` (`simple_setup`, `simple_box`, `avian_2d`) and
  `references/lightyear/book/src/SUMMARY.md` — targets Bevy 0.19.
- `references/avian/crates/avian2d/examples/`.

Use the internet only when the snapshots are insufficient, prefer primary documentation, and record
why in the ticket.

## Ticket workflow

Ticket is the source of truth for planning and progress; the project code is `BRL`. Ticket is
young software: when the CLI misbehaves or could work better, file it with
`ticket task new --project TCK <title>`, not in `BRL`.

1. Search (`ticket list`, `ticket search`) before creating, and continue an existing ticket whose
   scope matches. One independently reviewable outcome per ticket: the description states the
   outcome, and the spec owns scope, decisions, acceptance criteria, constraints, verification, and
   playtest needs.
2. Statuses are `idea`, `backlog`, `todo`, `doing`, `done`, and `canceled`. Move to `doing` when
   work starts. `todo` is work still to do.
3. Inspect local source, docs, and references before guessing. Record research findings and
   consequential decisions in the spec, and record material scope or architecture changes there
   before implementing. If a choice exceeds the owner's existing direction, add a Ticket question
   and wait for the answer.
4. Implement without silently broadening scope; file deferred work as a linked backlog ticket.
5. Verify in proportion to risk. Record commands, results, and anything manual or unavailable in
   the ticket.
6. If a native or subjective playtest is needed, put the run path, controls, scenario, known limits,
   and what to observe in the spec. Don't hold the ticket in `doing` waiting for it.
7. Record every feedback item as implemented, deferred (with its ticket), rejected (with the reason),
   or awaiting evidence.
8. For substantial work, record mistakes, causes, and prevention in the ticket. Create or improve a
   skill only for a lesson that recurs.
9. Move to `done` once the implementation, the agent's own verification, durable docs, and learning
   are complete. The owner and other testers test `done` work and reopen it or file a new ticket.
   Run `ticket sync` before handoff and report the ticket ID. If sync reports a conflict, stop and
   surface it; never choose a side.

Change tickets only through `ticket task <ID> desc|spec|comment|question|link|move`. Never edit
`.ticket/` or `docs/tasks/` (Ticket-owned state and read-only mirrors). Preserve unrelated user
changes.
