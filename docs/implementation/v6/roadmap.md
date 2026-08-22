# Version 6 implementation roadmap

## Purpose and scope

V6 begins gameplay expansion by making combat balance iteration fast and authoritative. Its first
vertical slice is a development-only Balance Lab: a local server operator opens a small web console
for one routed Practice worker, edits declared fighter and weapon values, and applies one validated
revision through the same fixed-tick server simulation used by multiplayer.

V6 does not turn arbitrary constants into mutable reflection data. Engine ceilings, identifiers,
wire schemas, recipe topology, collision geometry, build economy, abilities, passives, and match
rules remain outside M01.

The durable operator workflow, validation philosophy, and property-system maintenance contract now
live in the [Balance Lab guide](../../15-balance-lab.md); this roadmap retains delivery status and
historical scope.

## Version status

| Field | Value |
|---|---|
| Status | Complete |
| Current milestone | Complete — M01 accepted and closed on 2026-08-22 |
| Entry gate | Satisfied: V5 completed and was accepted on 2026-08-22 |
| Completion gate | A tuning-enabled Practice worker serves the local console, validates and applies shared combat tuning atomically, resets coherently, and passes canonical isolation and routed verification |

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Milestone overview

| Milestone | Status | Player/developer-visible deliverable | Plan |
|---|---|---|---|
| 01 | Complete | Stable loopback Vite/React Balance Lab with locally persisted fighter and weapon tuning across routed Practice workers | [milestone-01.md](./milestone-01.md) |

## Initial V6 backlog

| ID | Item | Disposition |
|---|---|---|
| V6-BALANCE-EXPORT | Export a reviewed RON patch/change summary | Deferred; M01 persistence remains an internal validated snapshot, not authored-content export |
| V6-BALANCE-DISCOVERY | Stable loopback endpoint and cross-Practice override persistence | Accepted into M01 after native playtest feedback |
| V6-BALANCE-REMOTE | Stable supervisor discovery and authenticated remote access | Deferred; M01 is loopback-only |
| V6-BALANCE-HOT-APPLY | Apply compatible weapon changes without a full reset | Deferred; M01 applies transactionally with reset |
| V6-BALANCE-ABILITIES | Tune ultimate and passive values | Deferred outside M01 |
| V6-BALANCE-TELEMETRY | Charts, comparisons, and richer balance evidence in the console | Deferred; M01 shows only derived recipe facts |
