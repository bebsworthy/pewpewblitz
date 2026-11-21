# Outcome
On the public server, offensive names are refused, an operator can kick or ban an account without a restart, and no single source can flood storage with accounts or maps.

# Owner decision
Open public release (q-tzc8kxb), so name moderation is required at launch (recorded 2026-11-21).

# Found (2026-11-21)
## Names
- Display names are validated structurally only: length, control characters and undrawable code points (`src/lobby.rs:455-476`: `normalize_proposed_display_name`, `normalize_new_display_name`).
- They're proposed in every `LobbyHello`, shown to other players, and stored as `author_display_name` on published maps.
- Map names are bounded to ≤ 96 bytes.
- No content filter exists. The `InvalidName` rejection already gives the client an "Edit name" action (`packages/brawler-client/src/flow/reducer.rs:745`).

## Existing limits
- Map publishing: `pending_per_account: 3`, `submit_cooldown_seconds: 120`, manual approval (`moderation: Approve`, in `config/server/game-types.ron`); `MAX_PENDING_PUBLISHED_MAPS = 64` server-wide (`src/profiles/storage/published_maps.rs:15`).
- Queue commands: a token bucket of 4 (`src/server/lobby/queue.rs:16`).
- Pre-auth ingress limits per source (`packages/brawler-routing/src/limits.rs`).
- Lobby sessions capped (BRL-fdbz).
- **Not limited:** new account creation. Any unknown key creates a row (`load_or_create`, `src/profiles/storage.rs`). Profile mutations aren't rate-limited.

## Operator tooling
- `brawler-profile-admin` has backup and map moderation only. The lobby re-reads approved maps on an interval, so an operator decision takes effect without a restart (`src/server/lobby/map_publishing.rs:12`). Bans can follow the same pattern.

# Decisions (owner-approved 2026-11-21)
1. **Name filter.**
   - An operator-editable blocklist file (`config/server/name-blocklist.txt`, validated and size-bounded, re-read on the same interval as approved maps).
   - Matching is on a normalised form: case-folded, with separators, digits-as-letters and repeated characters collapsed.
   - It applies to new display names at lobby admission (rejected as `InvalidName`) and to map names at submission.
   - It ships with a small starter list. The owner can extend it, and no third-party filtering service is used.
2. **Ban and kick.**
   - `brawler-profile-admin accounts ban <reference> [--reason]` and `unban` write a `bans` table; `accounts kick <reference>` writes a one-shot kick. The account reference comes from BRL-tysh.
   - The lobby re-reads both on its interval. It disconnects a banned or kicked session and rejects a banned account's admission with a new `LobbyJoinRejection::Banned` (one protocol bump), shown to the player as "This account is banned from this server."
   - A ban also withdraws the account's pending and approved maps.
3. **Account-creation bound:**
   - The lobby limits new account creations server-wide, for example 300 per hour (operator config with a code ceiling).
   - The token service (BRL-z277) limits token requests per source IP, for example 30 per minute.
   - Past either limit, the player gets a retryable `ServerFull`-style message, never a silent drop.
4. **Profile mutation bound:** a per-session token bucket for profile writes, like the queue's.
5. **Deferred:** an in-game "report player" path goes into a backlog ticket; launch relies on the filter, operator review of published maps, and bans.

# Scope
Decisions 1–4, with tests: blocklist normalisation; ban rejection at admission and disconnect of a live session; creation bound; mutation bucket.

# Acceptance
- A blocked name gets `InvalidName`.
- A banned account is disconnected within one re-read interval and refused afterwards.
- The 301st account creation within an hour is refused with a retryable message (test with a small configured limit).

# Verification
`just lint`, `just test`, and a network test for ban plus disconnect.
