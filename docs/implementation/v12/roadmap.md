# Version 12 implementation roadmap

## Purpose and scope

V12 replaces Feature Yard as the sole 3v3 play surface with one deliberate player-facing map for
Wipeout, Hot Zone, and Heist, then improves the development balancing workflow and iterates fighter
and weapon balance from gameplay feedback. Feature Yard remains available where its integration
coverage is still useful.

The user selected the three M01 layout references on 2026-08-26 and directed manual Brawler-native
map authoring without the external image importer. The user also fixed the server-owned map
dimension policy at a 20-cell minimum and 512-cell maximum on each axis. Balance Lab requirements
belong to M02. On 2026-08-26 the user defined its first scope as correcting the editor contract and
improving presentation/ease of use. Balance analysis and decision-support features remain later
work. Real-time application is researched in M02 but is not authorized for implementation yet.

M01 also removes style-specific 512-placement and 128-concealment limits. Capacity follows the
four mutually exclusive asset slots per cell, concealment may cover every cell, and resolved
snapshots remain bounded at 32 MiB. Extreme-density rendering and lookup optimization are deferred
until measured maps require them; this does not narrow the authoring contract.

The first native map review found that imported environment scenes did not consistently agree with
their authoritative cell footprints. M01 closed that feedback by enforcing fitting policies from
intrinsic scene bounds and replacing the three maps' solid-block approximations with a small
KayKit Block Bits family. The user accepted the corrected presentation on 2026-08-26. This was
map-playtest polish, not M02 balancing-tool work.

M01 reopened later on 2026-08-26 for bounded framing, fighter-footprint, pitch, and silhouette
corrections. Camera fit/follow is now decided independently per axis from each map's dimensions and
the current viewport's conservative visible rectangle. The user accepted the corrected native
presentation and requested the final commit on 2026-08-26, closing M01 and making M02 next.

## Version status

| Field | Value |
|---|---|
| Status | Implementing |
| Current milestone | M02 — Balance Lab correctness and operator presentation |
| Entry gate | Satisfied: V11 completed and was accepted on 2026-08-26 |
| Completion gate | Three accepted 3v3 maps, the user-defined M02 balancing workflow, and feedback-driven fighter/weapon tuning pass their owning automated, routed, native, feedback, documentation, and learning gates |

## Milestone overview

| Milestone | Status | Player-visible deliverable |
|---|---|---|
| 01 | Complete | One manually authored, grid-readable 3v3 map each for Wipeout, Hot Zone, and Heist, accepted through native playtest, with per-axis framing and a matched one-cell fighter footprint |
| 02 | Feedback review | [Correct Balance Lab field contracts, a clearer operator presentation, and authoritative player-loadout context; balancing assistance remains deferred](./milestone-02.md) |
| 03 | Not started | Character and weapon balance/rework iterated from gameplay feedback on the accepted maps |

## Ordering

M01 came first so gameplay and map feedback were not shaped around balancing-tool features the user
had not requested. The user has now specified M02's correctness and ease-of-use workflow. M03 uses
actual map play rather than speculative tuning targets.

M01 completed on 2026-08-26 after the three maps, dimension/capacity policy, half-cell
Hot Zone anchor, destructible cactus, intrinsic scene fitting, KayKit visual family,
automated/routed/native evidence, feedback disposition, documentation reconciliation, and
learn-from-errors review passed. Its later framing, one-cell fighter-footprint, pitch, silhouette,
and overhead-anchor corrections passed the affected automated/routed checks and were accepted in
the final native review before commit `8e7f751`.

## Immediate gameplay feedback applied

The user initially queued these values for M03, then explicitly directed that they be applied
immediately during M02 feedback review:

- Default movement speed: `100` world units per second.
- Lightweight movement speed: `110` world units per second.
- Reinforced movement speed: `90` world units per second.

These absolute authored values replace `320`/`360`/`288`, preserve the requested
100%/110%/90% relationship, and advance the build balance revision to `3`. The narrow correction
does not begin the wider M03 character and weapon rework.

## Version boundaries

- M01 does not redesign Balance Lab, tune fighters or weapons, add a map editor, or introduce new
  gameplay-object families. Presentation-only wall and cover variants may reuse existing gameplay
  profiles when a supplied map reference demonstrates a distinct visual role.
- M02 fixes correctness and ease of use only. It does not add balancing analytics, telemetry,
  automated recommendations, or live gameplay mutation without a separate user decision.
- M03 changes canonical balance only through recorded gameplay feedback and affected verification.
- Server authority, sparse recipes, stable identities, typed mode anchors, routed admission,
  Practice bots, and the sole 3D renderer remain the production paths.
