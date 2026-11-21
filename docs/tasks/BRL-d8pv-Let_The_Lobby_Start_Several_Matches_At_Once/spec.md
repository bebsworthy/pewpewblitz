# Outcome
Players starting Practice or forming queue matches at the same moment each get a match, up to the host's free match slots, instead of one start blocking every other for up to 30 s.

# Found (2026-11-21)
- **The lobby holds exactly one formation at a time.**
  - `LobbyState` has single-slot fields: `pending`, `allocation_completed`, `active_allocation`, `allocation_rejected` and the `product_*` flags (`src/server/lobby/mod.rs:320-330`).
  - `ProductFormationState` has `active: Option<MatchReservationId>`, `practice: Option<PracticeFormation>` and one `loading_deadline` / `grant_deadline` (`src/server/lobby/mod.rs:1130-1145`).
  - Practice returns `Busy` while any start is pending (`mod.rs:778`), and queue formation waits on `formation.active` or any reservation (`mod.rs:2024`).
- **The supervisor already supports concurrent allocations:** `allocations: HashMap<RequestId, AllocationRecord>` (`packages/brawler-routing/src/runtime/mod.rs:311`). The lobby manifest advertises `outstanding_allocations: 2` (`supervisor.rs:83`), validated non-zero (`manifest.rs:295`).
- **Measured** (BRL-fdbz run 1): four Practice starts within 0.1 s gave 1 match and 3 `Busy` rejections.

# Decisions (owner-approved 2026-11-21)
1. Replace the single-slot formation fields with a bounded map keyed by reservation (or request) ID. Each entry holds its own allocation state, grants and loading and grant deadlines. Bound it by `min(outstanding_allocations, free_match_slots)`.
2. Practice returns `Busy` only when that bound is reached; the existing capacity message covers zero free slots.
3. Queue formation may proceed while other formations are pending, within the same bound. Fairness between queue and Practice is first come, first served on the lobby tick.
4. Raise `outstanding_allocations` to 4 (or tie it to BRL-fdbz's `--max-matches`), checked against the supervisor's existing bounds.
5. Cancellation, rejoin seats and grant forwarding stay per formation; schedule tests cover the ordering of concurrent completions.

# Scope
Decisions 1–5, with lobby `App` tests (two formations progressing independently; one cancelling while another completes; bound reached gives `Busy`) and a real-process test.

# Acceptance
A real-process test starts four Practice matches within one lobby tick window with four free slots, and all four reach Active. A queue match forms while a Practice start is pending. `just practice-e2e`, `just e2e 2` and `just rejoin-e2e` still pass.

# Verification
`just lint`, `just test`, the e2e runs above, and the BRL-fdbz capacity script with zero stagger.
