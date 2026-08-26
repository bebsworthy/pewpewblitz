# Version 12 implementation roadmap

## Purpose and scope

V12 replaces Feature Yard as the sole 3v3 play surface with one deliberate player-facing map for
Wipeout, Hot Zone, and Heist, then improves the development balancing workflow and iterates fighter
and weapon balance from gameplay feedback. Feature Yard remains available where its integration
coverage is still useful.

The user selected the three M01 layout references on 2026-08-26 and directed manual Brawler-native
map authoring without the external image importer. The user also fixed the server-owned map
dimension policy at a 20-cell minimum and 512-cell maximum on each axis. Balance Lab requirements
belong to M02 and remain intentionally unspecified until the user defines them.

M01 also removes style-specific 512-placement and 128-concealment limits. Capacity follows the
four mutually exclusive asset slots per cell, concealment may cover every cell, and resolved
snapshots remain bounded at 32 MiB. Extreme-density rendering and lookup optimization are deferred
until measured maps require them; this does not narrow the authoring contract.

The first native map review found that imported environment scenes do not consistently agree with
their authoritative cell footprints. M01 therefore remains open for one presentation correction:
enforce the existing fitting policies from intrinsic scene bounds and replace the three maps'
solid-block approximations with a small KayKit Block Bits family. This is map-playtest polish, not
the M02 balancing-tool work.

## Version status

| Field | Value |
|---|---|
| Status | User playtest |
| Current milestone | M01 — three proper 3v3 maps |
| Entry gate | Satisfied: V11 completed and was accepted on 2026-08-26 |
| Completion gate | Three accepted 3v3 maps, the user-defined M02 balancing workflow, and feedback-driven fighter/weapon tuning pass their owning automated, routed, native, feedback, documentation, and learning gates |

## Milestone overview

| Milestone | Status | Player-visible deliverable |
|---|---|---|
| 01 | User playtest | One manually authored, grid-readable 3v3 map each for Wipeout, Hot Zone, and Heist, accepted through native playtest |
| 02 | Not started | Balancing-tool improvements defined by the user after playing the M01 maps |
| 03 | Not started | Character and weapon balance/rework iterated from gameplay feedback on the accepted maps |

## Ordering

M01 comes first so gameplay and map feedback are not shaped around balancing-tool features the user
has not requested. M02 begins only after the user specifies its operator workflow. M03 uses actual
map play rather than speculative tuning targets.

## Version boundaries

- M01 does not redesign Balance Lab, tune fighters or weapons, add a map editor, or introduce new
  gameplay-object families. Presentation-only wall and cover variants may reuse existing gameplay
  profiles when a supplied map reference demonstrates a distinct visual role.
- M02 scope is not inferred from the existing Balance Lab backlog.
- M03 changes canonical balance only through recorded gameplay feedback and affected verification.
- Server authority, sparse recipes, stable identities, typed mode anchors, routed admission,
  Practice bots, and the sole 3D renderer remain the production paths.
