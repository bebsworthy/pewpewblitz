# Brawler repository guide

## Quick orientation

Brawler is an original, cross-platform top-down arena shooter built around player-authored fighter builds. Combat readability, meaningful build tradeoffs, short matches, reusable content primitives, and server-authoritative networking are the core product constraints.

Start with:

1. `docs/00-product-direction.md` for product intent and non-goals.
2. `docs/05-gameplay-mvp.md` for v1 gameplay scope and acceptance criteria.
3. `docs/08-network-architecture.md` for authority and replication boundaries.
4. `docs/implementation/v1/roadmap.md` for current progress and milestone order.
5. The current `docs/implementation/v1/milestone-NN.md` for validated scope and tracked work.

## Technical stack

- Rust project with independently buildable macOS-client and dedicated headless-server application configurations; Milestone 01 decides the package, target, and Cargo-feature topology from evidence.
- Bevy 0.19 for ECS, application/plugin structure, rendering, input, assets, audio, and UI.
- Lightyear 0.29 for client/server transport, input networking, replication, interpolation, and later prediction/rollback where evidence justifies it.
- Avian 2D 0.7 only when collision queries, contact handling, or generated terrain colliders justify it.
- Fixed-tick, dedicated-server-authoritative simulation from the first gameplay code.
- macOS is the initial client development target; local dedicated-server and multi-client testing are required.

Use Bevy's `World` as the runtime gameplay model. Keep authored definitions, selected builds, runtime ECS state, networking registration, and client presentation distinct without assuming that each concern needs a crate or architectural layer. The dedicated-server configuration must exclude rendering, windowing, audio, device input, and client assets. Networked types use stable player, match, and definition IDs rather than exposing process-local ECS entity identity across the wire.

Group focused systems, components, resources, and messages into cohesive plugins. Share a gameplay system between server and client only when it genuinely executes in both places, such as measured client prediction. Keep server-only authority rules on the server and presentation systems on the client. Create another package or public API only for a demonstrated feature-isolation, platform, compile-time, testing, or reuse boundary.

For architecture decisions, prioritize Brawler's gameplay and authority requirements, the local Lightyear material, verified Bevy 0.19 APIs, and Bevy-native patterns before general Rust architecture advice. Server-oriented DDD or hexagonal architecture is not the governing model; use ports/adapters only at a concrete external boundary where they solve an observed problem.

## Local implementation references

Use the checked-in source and examples before guessing an API or copying an unrelated internet snippet, but verify snapshot versions before transferring exact APIs:

- `references/bevy/examples/` — official Bevy example source. Start with `README.md`, then locate focused examples with `rg`; useful foundation examples include `app/headless.rs`, `app/plugin.rs`, and `app/plugin_group.rs`.
- `references/lightyear/examples/` — official Lightyear example projects and their `Cargo.toml` feature sets. Start with `README.md`; use `simple_setup` for minimal client/server composition, `simple_box` for authoritative replication/prediction/interpolation, and `avian_2d` only when physics integration is in scope.
- `references/lightyear/book/` — local Lightyear book. Start with `src/SUMMARY.md`, then read the relevant tutorial or concept pages for protocol, transport, replication, inputs, system ordering, shared plugins, client/server setup, prediction, interpolation, and Avian integration.
- `references/avian/crates/avian2d/examples` — official Avian 2D examples project and their `Cargo.toml` feature sets. 

The Lightyear 0.29 snapshot targets Bevy 0.19, while the checked-in Bevy source is currently 0.20-dev. Use the Bevy snapshot for architectural examples, but confirm exact APIs against Bevy 0.19 source or official documentation before implementation.

Treat `references/` as read-only upstream material unless the user explicitly requests a snapshot update. Inspect the example README, source, and `Cargo.toml` together because feature flags and application topology are part of the example. Adapt the smallest relevant pattern to Brawler's authority and dependency boundaries; do not copy whole examples blindly.

When research still requires the internet, prefer current primary documentation and record why the local snapshot was insufficient.

## Versioned implementation docs

Implementation work lives under `docs/implementation/<version>/`:

```text
docs/implementation/
  v1/
    roadmap.md
    milestone-01.md
    milestone-02.md
    ...
  v2/
    roadmap.md
    milestone-01.md
    ...
```

`roadmap.md` defines version scope, ordering, delivery gates, status, and backlog. Each `milestone-NN.md` records the research, user-validated technical specification, implementation checklist, test evidence, playtest handoff, feedback decisions, and closeout learning for one milestone.

Create a milestone file when that milestone becomes next. Do not pre-author distant technical designs that should incorporate earlier evidence.

Allowed roadmap statuses are `Not started`, `Researching`, `Specification review`, `Implementing`, `Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Milestone process

For the next non-complete milestone:

1. Update the roadmap and milestone status to `Researching`.
2. Inspect the relevant local Bevy/Lightyear references first, then research current primary sources, alternatives, compatibility, and risks. Record exact local paths and external links in the milestone file.
3. Write the technical specification, ECS ownership and lifecycle, plugin/schedule composition, network behavior, implementation tasks, test plan, visual checks, and exit criteria.
4. Set the status to `Specification review` and deliver the specification to the user. Do not begin production implementation until the user validates it.
5. Set the status to `Implementing` and complete the tracked tasks without silently expanding milestone scope.
6. Set the status to `Verifying` and run unit tests, integration tests, local network tests, and visual/controller checks required by the specification.
7. Set the status to `User playtest` and provide a clear build/run path, controls, scenario, known limitations, and requested observations.
8. Set the status to `Feedback review`. For each feedback item, record whether it is implemented now, deferred to the version backlog, rejected with rationale, or awaiting more evidence.
9. Re-run affected verification after accepted changes.
10. Perform a learn-from-errors review. Record mistakes, causes, prevention, and reusable lessons. Create or improve project/Codex skills when the learning is recurring and genuinely reusable.
11. Mark the milestone `Complete` only after exit criteria, evidence, user feedback triage, and the learning review are complete. Update the roadmap current milestone.

## Implementation and verification rules

- The current milestone file is the implementation scope contract. Update and revalidate it before materially changing scope or architecture.
- Server authority is not optional, including in-process and offline development modes.
- Clients send intent, not positions, hits, damage, scores, status triggers, or terrain edits.
- Separate authored definitions, selected builds, and runtime state.
- Keep gameplay events independent from rendering, audio, camera, and HUD presentation.
- Use focused pure-function tests where a rule is naturally independent of ECS. Test component, resource, lifecycle, and state behavior with small `App`/`World` schedule tests; add headless integration tests for authority and replication.
- Advance Bevy fixed time or explicitly run the relevant schedule in time-dependent tests rather than waiting on wall-clock sleeps.
- Visual verification complements automated tests; it does not replace them.
- Preserve unrelated user changes and keep deferred work visible in the active version backlog.

Do not invent build or test commands before the Cargo project exists. Milestone 01 must establish and document the canonical commands here or in the root README when implementation begins.
