# Technical specification

## Outcome and scope

Extract the cohesive profile-backed lobby transaction adapter currently embedded in `src/server/lobby/mod.rs` into one private `src/server/lobby/profile_transactions.rs` module. The module owns:

- authenticated `ClientHello` intake through asynchronous profile-load admission;
- pending admission correlation and cleanup;
- completed profile load and mutation outcome processing;
- authenticated `ProfileCommand` intake, queue-lock sampling, authority submission, and exact `ProfileOutcome` mapping.

This is an organization and characterization phase. It must not change protocol types, compatibility negotiation, profile storage/authority implementation, queue policy, lobby identity, admission payloads, system ordering, deferred-command visibility, or player-visible behavior.

## Target ownership

Move into the private adapter:

- `PendingLobbyAdmissions` and private `PendingLobbyAdmission`;
- `lobby_receive_hellos`;
- `process_profile_storage_results`;
- `collect_profile_commands`;
- exhaustive profile request-ID and rejection/error mapping helpers used only by those systems.

Keep in the lobby composition root:

- `LobbyPlugin` and `LobbyScheduleSet` composition;
- `LobbyState`, `LobbyClient`, queue/Practice state and systems;
- startup initialization, disconnection cleanup, queue snapshot publication, match formation/publication;
- the shared `authenticated_netcode_id` helper because the Netcode authentication observer also consumes it.

Expose only `PendingLobbyAdmissions` and the three schedule-facing systems to the parent with the narrowest visibility. Do not add a sub-plugin, generic transaction framework, service traits, new messages/resources, or a second profile-authority poller.

## Required ordering and visibility

Preserve the outer Update chain exactly:

`BeginLobbyFrame -> AuthenticateLobbyHellos -> ApplyProfileTransactions -> CollectQueueClientMessages -> Cleanup -> ApplyQueueTransactions -> Form -> Publish`.

Preserve the inner profile chain exactly:

`process_profile_storage_results -> collect_profile_commands -> ApplyDeferred`.

These relationships are behavioral contracts:

- loads and mutation outcomes from one `ProfileAuthority::poll_loads()` call are processed before another buffered command is submitted;
- a newly accepted load's deferred `LobbyClient` insertion is not visible to profile command collection in that update;
- the insertion is visible to queue collection/cleanup after the trailing flush;
- profile commands are handled before queue commands, so an existing ticket locks mutation while a not-yet-queued same-update mutation retains precedence over a new Join.

## Preserved admission behavior

- Validation/rejection order remains connected link, authenticated Netcode ID, routed peer identity, global compatibility, existing accepted session replay suppression, pending identical replay suppression, changed pending Hello rejection, and profile load submission.
- Existing-session and identical-pending replays remain silent.
- Pending changed Hello remains `InvalidAccount`.
- `begin_load` error mapping remains AccountInUse -> AccountInUse, StorageStopped -> StorageUnavailable, every other error -> InvalidAccount.
- Load rejection mapping remains StorageFault -> StorageUnavailable and every other profile decision -> InvalidAccount.
- Missing pending entry/entity, disconnect or identity mismatch, load rejection, or lobby admission rejection removes profile-authority ownership exactly once.
- Successful promotion retains the exact `LobbyClient`, authenticated control fact, one-shot welcome gate/payload, and initial queue snapshot publication.

## Preserved command behavior

- Process completed loads before completed mutations from the same poll; mutation delivery still targets only the first connected, non-disconnected matching `RemoteId`.
- Receive at most four profile commands per client per update.
- Ignore disconnected clients and derive `queue_locked` from the pre-existing `QueueState::ticket_for_client` result.
- Submit commands sequentially and retain the original command only for immediate error/request-ID response.
- Preserve Pending/no-send, Immediate/send, StorageStopped/fatal panic, QueueLocked/QueueLocked, InvalidRequest-or-UnknownSession/InvalidRequest, and all other errors/TemporarilyUnavailable mappings with `snapshot: None`.
- Preserve exhaustive request-ID extraction across every existing `ProfileCommand` variant.
- Do not introduce sorting, drain changes, broader query filters, or a `LobbyClient` requirement for async mutation outcome delivery.

## Tests and verification

Add focused characterization only where it creates a durable seam:

1. exhaustive request-ID coverage for all profile command variants;
2. table coverage for profile load rejection and command submission error mappings, while retaining StorageStopped as the fatal system boundary;
3. existing lobby owned-ambiguity and production schedule tests remain green; source-level review confirms the unchanged outer chain and inner `.chain()`/`ApplyDeferred` boundary;
4. existing profile-authority and routed lobby/queue tests cover serialized mutations, existing-ticket queue lock, admission, and queue visibility without duplicating the storage matrix.

Run:

- `cargo fmt --all -- --check`
- `git diff --check`
- focused server lobby/profile tests
- `cargo check --locked --no-default-features --features server --all-targets`
- `cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings`
- `just check`
- `just lint`
- `just test`

Native evidence is not required if all protocol, copy, timing, schedule, and visible outcomes remain unchanged.

## Exclusions

Profile storage redesign, authority API changes, queue semantics, Hello compatibility changes, protocol revisions, public API changes, startup decomposition, queue/match extraction, new persistence policies, additional command variants, and client work are excluded.

## Implementation and closeout — 2026-08-31

Implemented one private `src/server/lobby/profile_transactions.rs` adapter and kept `src/server/lobby/mod.rs` as the visible plugin/schedule composition root.

The adapter now owns pending profile-backed admissions, authenticated Hello intake, the single profile-authority result pump, authenticated profile command intake, and exact wire rejection/decision mappings. Only `PendingLobbyAdmissions` and the three schedule-facing systems are exposed to the parent with `pub(super)`; the pending record and mapping helpers remain private. Startup, LobbyState/QueueState, disconnection cleanup, queue/match systems, publication, and the shared Netcode identity helper remain in the parent.

Behavioral ordering is unchanged and was verified by scoped diff/source review:

- the outer Update set chain remains BeginLobbyFrame -> AuthenticateLobbyHellos -> ApplyProfileTransactions -> CollectQueueClientMessages -> CleanupDisconnectedSessions -> ApplyQueueTransactions -> FormReservations -> PublishQueueOutcomesAndSnapshot;
- the inner chain remains `process_profile_storage_results -> collect_profile_commands -> ApplyDeferred`;
- exactly one `ProfileAuthority::poll_loads()` remains, with loads processed before mutation outcomes and before new command collection;
- deferred LobbyClient insertion remains invisible to same-update profile collection and visible to later queue/cleanup phases;
- existing queue tickets lock profile mutation while a new same-update queue Join remains ordered after profile command collection.

Focused tests exhaust all five ProfileCommand request-ID variants, all twelve ProfileDecision load mappings, and all nine ProfileAuthorityError command/join mappings. StorageStopped remains an explicit fatal system arm. Existing lobby/profile/routed tests continue to cover replay suppression, admission, serialized authority mutations, queue locks, initial queue visibility, disconnect cleanup, and routed convergence.

Verification:

- `cargo fmt --all -- --check` — pass.
- `git diff --check` — pass.
- `cargo test --locked --no-default-features --features server --lib server::lobby` — 41 passed, 452 filtered.
- `cargo check --locked --no-default-features --features server --all-targets` — pass.
- `cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings` — pass.
- `just check` — pass.
- `just lint` — pass.
- `just test` — pass: routing package/process targets, client 545, server 493, Balance Lab 522, cross-feature network smoke 1, routed network 97, performance 12.

Independent review found no P0, P1, or P2 issue. Native evidence was not required because the phase changes no protocol, timing, copy, UI, schedule result, or player-visible behavior.

Learn-from-errors review: the first strict-Clippy pass caught a pure mapping helper taking ProfileDecision by value and the initial block extraction accidentally left the following Bevy system without its existing narrow `needless_pass_by_value` allow. Both were corrected. The reusable lesson is to review both sides of a moved source range: item-level attributes immediately after the extracted block are part of the retained item's contract, and pure mapping seams should borrow when ownership adds no value.
