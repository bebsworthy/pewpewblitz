# Technical specification

## Outcome and scope

Refactor only the client customization branches currently owned by `client::flow::reducer`. Move brawler/profile actions and weapon-equipment actions into focused private reducer submodules while retaining:

- one schedule-facing `resolve_flow_action` system;
- the exact explicit -> session -> ordinary action precedence;
- profile-decision reconciliation before every action category;
- one existing `FlowCommit` applied by the existing commit phase;
- the current Update schedule and ApplyDeferred boundary.

This is an organization and characterization phase. It must not change profile protocol commands, local/network models, UI copy, rendered screens, overlay/focus behavior, queue/Practice locks, settings, connection, matchmaking, result navigation, schedule order, or player-visible behavior.

## Current problem

`src/client/flow/reducer.rs` is approximately 1,800 lines. Its `resolve_flow_action` coordinator and sibling helpers jointly own connection/persistence, session observations, shell overlays, game selection, profile-decision reconciliation, brawler creation/edit/delete/select, physical weapon-part equipment, matchmaking/Practice/results navigation, teardown, and final commit materialization.

The existing one-system precedence boundary is correct and must remain. The ownership problem is that customization rules and drafts are implementation details inside the same root as network/session and match-flow reducers, making changes difficult to review and testing seams implicit.

## Target module ownership

Add private modules under `src/client/flow/reducer/`:

### profile.rs

Own:

- `PendingCreatedBrawler` and `PendingEditedBrawler`;
- profile-decision reconciliation and exact notice/inline-error/overlay/focus results;
- create/open/confirm/cancel brawler creation;
- select brawler;
- open/cancel/confirm brawler editing;
- name/ultimate/passive editing helpers;
- delete confirmation and confirmed deletion;
- profile mutation lock checks shared with customization actions;
- brawler list/details customization overlay and focus transitions where they are part of profile action outcomes.

### equipment.rs

Own:

- opening weapon equipment for a brawler;
- selected slot updates;
- physical-part uniqueness and compatibility validation;
- equip, unequip, confirm, and cancel actions;
- exact equipment inline errors;
- BrawlerDetails/BrawlerList overlay and focus outcomes owned by equipment actions.

Keep `reducer.rs` as the private composition and precedence surface. It continues to own connection/shell and matchmaking/results concerns until their own later tickets, and continues to own `teardown_session` and `commit_flow`.

## Dispatch and access rules

- Keep `resolve_flow_action` as the only Bevy system involved in action resolution.
- Plain helper reducers may receive focused model/draft references and write only through existing domain models plus `FlowCommit`; do not add systems, resources, messages, events, commands, observers, action buses, or nested commits.
- Keep default visibility private; use `pub(super)` only for the minimal helper/resource surface consumed by the parent reducer or flow composition.
- Keep `FlowUiAction`, `PendingFlowActions`, `FlowCommit`, `OverlayCommit`, `ClientFlow`, `ClientOverlay`, and protocol/profile public paths unchanged.
- Keep profile decision processing at the exact current first position, before explicit/session/ordinary arbitration.
- Keep explicit Cancel/Disconnect/ConfirmChangeServer preemption exact.
- Avoid broad wrapper contexts that mutably dereference unrelated resources or cause spurious Bevy change detection.
- Do not split by one function/type per file and do not move connection or matchmaking logic in this phase.

## Preserved behavior

- Accepted creation locates the creation ordinal, publishes `Created <name>.`, opens BrawlerDetails, and focuses index 0.
- Accepted edit opens BrawlerDetails and focuses index 1.
- Rejected create/edit restores the matching draft overlay and exact inline error.
- Queue, Practice, and pending-profile locks remain exact for create/edit/delete/select/equipment actions.
- Re-selecting the currently selected brawler remains mutation-free.
- Profile commands and revisions remain unchanged.
- Physical parts remain unique across saved brawlers; conflicting equipment produces the exact current inline error and no profile command.
- Cancel/confirm equipment preserve exact details/list overlay destinations.
- All existing dashboard build focus and list/detail transitions remain unchanged.
- No new network, storage, profile, schedule, rendering, or presentation path.

## Tests and verification

Retain and add focused coverage for:

1. profile decision reconciliation still runs before explicit action preemption;
2. accepted/rejected create and edit decisions preserve exact notice, overlay, focus, and draft errors;
3. create/edit/delete/select actions are ignored under queue, Practice, and pending-profile locks;
4. re-selecting the current brawler emits no mutation;
5. invalid equipment slot is ignored;
6. a physical part equipped on another brawler produces the exact inline error and no profile request;
7. equip/unequip/confirm/cancel preserve exact draft and overlay targets;
8. the client flow Update schedule has no owned ambiguities and retains Begin -> Observe -> Collect -> Resolve -> Teardown -> ApplyDeferred -> Commit -> Present order.

Run:

- `cargo fmt --all -- --check`
- `git diff --check`
- focused client flow tests
- strict client all-target Clippy with `-D warnings`
- `just check`
- `just lint`
- `just test`

Native evidence is not required if UI copy, rendered structure, interaction results, focus, and state transitions remain unchanged.

## Exclusions

Connection/session observation extraction, matchmaking/Practice/results extraction, FlowCommit redesign, action precedence changes, profile protocol/storage changes, UI layout or copy changes, screen module decomposition, new settings/equipment features, and presentation work are excluded.

## Implementation and closeout — 2026-08-31

Implemented the customization reducer boundary as an organization-only change:

- `src/client/flow/reducer.rs` remains the sole schedule-facing action coordinator and owns the exact profile-decision -> explicit -> session -> ordinary precedence, `FlowCommit`, connection/match-flow branches, teardown, and commit materialization.
- Private `profile.rs` now owns pending create/edit correlation, decision reconciliation, list/detail/create/select/edit/delete actions, exact notices, draft errors, overlays, focus outcomes, and the existing entry-action lock predicate.
- Private `equipment.rs` now owns open/slot/equip/unequip/confirm/cancel behavior and the existing local cross-brawler physical-part uniqueness check. Compatibility/inventory validation remains in the existing profile authority path; no new client-side rule was introduced.
- Pending resource paths used by flow composition are preserved through a narrow `pub(in crate::client::flow)` re-export; all other helper visibility is private or `pub(super)`.
- The Update schedule, `ApplyDeferred` boundary, protocol commands, UI copy, focus/overlay behavior, and player-visible behavior are unchanged.

Focused characterization proves profile reconciliation still precedes explicit action preemption; accepted create and edit preserve exact notice/details/focus 0/1 outcomes; rejected create and edit restore exact draft errors and overlays; queue-pending, Practice-pending, and profile-pending entry locks remain exact; invalid equipment slots are mutation-free; cross-brawler conflicts preserve the exact error and emit no request; equip/unequip/confirm/cancel preserve draft and overlay contracts. The existing owned-ambiguity test remains green, and source inspection confirms the unchanged Begin -> Observe -> Collect -> Resolve -> Teardown -> ApplyDeferred -> Commit -> Present composition.

Verification:

- `cargo fmt --all -- --check` — pass.
- `git diff --check` — pass.
- `cargo test --locked --no-default-features --features client --lib client::flow::tests -- --nocapture` — 47 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — pass.
- `just check` — pass.
- `just lint` — pass.
- `just test` — pass: routing 95 tests across its targets, client 545, server/Balance Lab 520, cross-feature network smoke 1, routed network 97, performance 12.

Independent review found no P0 or behavioral/code P1. Its characterization gap was resolved by adding exact accepted-edit and rejected-create assertions plus focus assertions for both accepted outcomes. Native evidence was not required because rendering, UI structure/copy, interaction results, and state transitions are unchanged.

Learn-from-errors review: the first strict-Clippy pass exposed a 116-line profile action dispatcher. The correction extracted the already cohesive creation sub-reducer instead of suppressing `too_many_lines`. The reusable lesson is to retain one Bevy schedule coordinator while decomposing only plain domain-action families, and to characterize intentional guard asymmetries before moving branches so a cleanup cannot silently broaden validation policy.
