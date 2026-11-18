# Outcome
While a disconnected player's match seat is held (BRL-yyeg) and a server bot drives their fighter,
every other player in the match can see it: the fighter's overhead name and the roster
(scoreboard, Zombie team strip) mark the seat as bot-controlled, and a short feed notice says when
the player dropped ("X disconnected - bot playing"), came back ("X is back") or did not return
("X left"). The authority model is unchanged: the server publishes one presentation fact and
clients never infer it.

# Current state (checked 2026-11-15 at HEAD 3437c0d8)
- Held state is server-only. `SeatHold { expires_at_tick }` (`src/server/seat_hold.rs:48`) and
  `RoutedSeat` (`:43`) are unreplicated components in the private `mod seat_hold`
  (`src/server/mod.rs:65`). `hold_released_seats` (`seat_hold.rs:256-316`, FixedUpdate,
  `GameplaySet::Lifecycle`, before `MatchSet::Lifecycle`) inserts `SeatHold` + a fresh
  `PracticeBotController` and removes `ControlledBy`/`ControlledSend`/`NativeBuffer` (BRL-4zx2);
  `reclaim_held_seat` (`:415-427`) removes `SeatHold` + the controller on rejoin;
  `expire_seat_holds` (`:318`) despawns the fighter at expiry. Nothing about the hold is sent to
  clients; a held fighter looks exactly like a connected one.
- `PublicParticipantState` (`src/matchplay/model.rs:172-183`) is the always-public roster
  projection, separate from the cullable fighter: `player_id`, `fighter_network_id`, `team_id`,
  `display_name`, `participant`, `selected`, `weapon_preset_id`,
  `status: PublicParticipantStatus` (`:162-170`, Alive/Ready/RestartReady/Defeated/Respawning/
  Protected). Registered `.replicate()` at `src/protocol.rs:622`. The server rebuilds it every
  frame in `concealment::server::sync_public_participant_projections`
  (`src/concealment/mod.rs:168-247`, PostUpdate before `ReplicationSystems::Send`, server
  feature), spawning one `Replicate::to_clients(NetworkTarget::All)` entity per fighter and
  despawning it when the fighter goes.
- `FighterDisplayName` is on the fighter, `replicate_once` (`src/protocol.rs:621`), so it cannot
  carry a changing fact.
- Protocol: `SUPPORTED_PROTOCOL_VERSION = 49` (`src/protocol.rs:47`). The registry fingerprint
  (`protocol_fingerprint`, `:788-801`) hashes registered types, not their fields: protocol 49
  (BRL-8y1f) changed a variant and left the fingerprint unchanged, so only the version guards a
  field change (docs/08-network-architecture.md:615-620).
- Client overhead plates: `presentation_3d/combat/fighter_ui.rs`. `OverheadFighterQuery`
  (`:76-93`) reads health, team, `FighterDisplayName`, weapon, boosts and `Controlled` from the
  fighter; `update_fighter_overhead_state` (`:618-699`) writes the name text and relation color
  (`overhead_name_color`, `:387`). It does not read `PublicParticipantState` or
  `NetworkEntityId` (the fighter carries `NetworkEntityId`, `src/protocol.rs:669`).
- Client roster: `hud.rs` `sync_roster_and_collect_phase_facts` (`:775-835`) copies each
  `PublicParticipantState` into `CachedRosterEntry` (`:59-67`, status `CachedRosterStatus`
  `:94-101`, `connected`). The text scoreboard `roster_entry_text` (`:1001-1030`) prints
  "alive"/"respawn N"/"disconnected"; the Zombie team strip `team_strip_rows` (`:551-580`)
  prints DOWN/Ns/+N/OFF. `connected=false` today means only "projection missing this frame".
  The text scoreboard is to be replaced by BRL-ejag (TODO).
- Client feed: `hud/kill_feed.rs` `KillFeed` (`:50-70`) is a bounded newest-first list of
  `KillFeedEntry { killer, victim, age }` (4 rows, 4 s), names and relation colors from
  `PublicParticipantState` (`:78`). There is no generic toast/notice surface in the match HUD.
- Tests: `tests/network/match_rejoin.rs` has separate-App scenarios with `active_routed_match`,
  `lose_session`, `rebind`, `reclaim` helpers (`:32-100`) and observes client 0/1 worlds; client
  HUD unit tests live in `hud.rs` (`:1281` roster text) and `hud/screen_tests.rs`.

# Plan
## Replicated fact (shared crate)
- Add to `src/matchplay/model.rs`:
  `pub enum ParticipantControl { Player, HeldByBot { expires_at_tick: u64 } }`
  (Serialize/Deserialize, Copy, Eq, `Default = Player`) and a field
  `pub control: ParticipantControl` on `PublicParticipantState`. Re-export it from
  `matchplay/mod.rs` beside `PublicParticipantStatus`. It is orthogonal to `status` (a held
  fighter can be Respawning or Defeated), so it is a separate field, not a status variant.
  Rationale for a field over a new marker component: the projection is always public and already
  the roster's single source, so overhead, roster and feed read one fact, and a concealed held
  enemy reveals nothing spatial. Practice bots that were bots from the start stay `Player`
  (out of scope; they are named as bots today).
- No new registration: `PublicParticipantState` stays `.replicate()`; the fingerprint is
  expected to stay unchanged (record before/after `routing-identity` to confirm).

## Server
- Make the hold readable by the projection: `pub(crate) use seat_hold::SeatHold;` in
  `src/server/mod.rs` (both are server-feature code). In
  `sync_public_participant_projections` add `Option<&crate::server::SeatHold>` to the fighter
  query and set `control = HeldByBot { expires_at_tick }` when present, else `Player`. The
  existing `*existing != state` guard sends it only on change (hold, reclaim). Expiry already
  despawns the fighter and therefore the projection.
- Ordering: the hold is inserted in FixedUpdate and the projection runs in PostUpdate before
  `ReplicationSystems::Send`, so the fact goes out in the same frame as the hold; no new sets.
- No change to the rebind/reclaim/expiry logic, IPC, control version or manifest.

## Protocol
- `SUPPORTED_PROTOCOL_VERSION` 49 -> 50 (field added to a replicated component changes the
  canonical encoding; the global handshake rejects 49 peers). Update the protocol tests that pin
  the version, docs/08 ("Protocol 50 adds `PublicParticipantState::control` ... fingerprint
  unchanged") and the docs/14 "Rejoining a running match" section (others are told).
  Every role must be rebuilt and restarted.

## Client presentation (`packages/brawler-client`)
- Shared lookup: a small `HeldSeats` resource (or a local `HashMap<u64 fighter_network_id,
  expires_at_tick>` built per frame) derived from `PublicParticipantState` for the current
  match, used by the overhead and HUD systems.
- Overhead (`fighter_ui.rs`): add `&NetworkEntityId` to `OverheadFighterQuery`; when the fighter
  is held, show a compact "BOT" chip before the name (a separate small text node in the name row,
  neutral/dim style, not the relation color) and dim the name color. Never shown on the local
  fighter (a rejoined local fighter is `Player` by construction; assert it in a test). Keep the
  120 px plate width assumption: the chip is at most 3 glyphs; if BRL-9k7q lands first, its
  truncation budget must include the chip (no hard dependency).
- Roster (`hud.rs`): `CachedRosterEntry` gains `held_until: Option<u64>`. `roster_entry_text`
  prints "bot Ns" (seconds left in the hold, from `expires_at_tick - now`) instead of the
  status when held; `team_strip_rows` shows "BOT" in the status slot. Distinct from today's
  "disconnected"/"OFF", which keep meaning "projection missing". BRL-ejag must carry the same
  marker into the new scoreboard (note it on BRL-ejag; no blocking dependency).
- Feed notices (`hud/kill_feed.rs`): add a notice entry kind to `KillFeed` (same bounded 4-row,
  4 s list, so no new HUD surface on a phone): one row with the participant's name in its
  relation color plus text. A `SeatControlTransitions` system keeps per-match last-seen
  `control` per player id and emits:
  - `Player -> HeldByBot`: "<name> disconnected - bot playing"
  - `HeldByBot -> Player`: "<name> is back"
  - a `HeldByBot` projection that disappears while the match is Active: "<name> left"
  The first observation of a participant (match join, the rejoining client's own first
  snapshot, a late concealment reveal) only records a baseline and emits nothing; the local
  player never gets a notice about itself; state resets on match id change. Strings live with the
  other HUD strings.

# Acceptance
- Network (`tests/network/match_rejoin.rs`, production match-worker graph, separate Apps):
  1. `a_held_seat_is_published_as_bot_controlled_to_the_other_players`: after `lose_session` of
     player one, client 1's world has player one's `PublicParticipantState` with
     `control == HeldByBot { expires_at_tick }` equal to the server `SeatHold` expiry; player
     two's own projection stays `Player`.
  2. After `rebind` + `reclaim`, both the rejoined client and client 1 observe `Player` for
     player one within a bounded number of steps; the rejoined client never observes its own
     seat as `HeldByBot` after its accepted join.
  3. Expiry: the projection disappears from client 1 (existing forfeit assertions unchanged).
  4. A Practice bot seat that was a bot from the start publishes `Player`.
- Server unit test (`concealment/tests.rs`): a fighter with `SeatHold` projects `HeldByBot`
  with the hold's tick; removing `SeatHold` projects `Player`.
- Client unit/ECS tests: `roster_entry_text` shows "bot Ns" for a held entry and the team strip
  shows "BOT"; the overhead name row shows the BOT chip only for a held non-local fighter
  (ECS test beside `plates_exist_only_while_the_fighter_is_replicated`); the transition system
  emits exactly the three notices above and nothing on a first observation, for the local player,
  or across a match id change.
- Protocol: version test pins 50; record `routing-identity` before/after (fingerprint expected
  unchanged); `just check`, `just lint`, `just test` pass.
- Device check (Pixel 8a vs a headless or second client, rebuilt protocol 50 roles, scratch
  supervisor): kill the other client mid-match; within ~10 s the phone shows the notice and the
  BOT chip over that fighter, the scoreboard/team strip marker counts down; relaunch the other
  client inside 60 s and see "is back" and the chip clear; let a hold expire and see "left".
  Check chip readability and plate overlap at 914x411 and 868x411.

# Out of scope
- Showing the held player their own hold, or the countdown on the rejoining client.
- Marking seats that were Practice bots from the start; any change to bot behavior while held.
- Results/Completed handling for a seat held at match end (BRL-jxfy).
- The scoreboard redesign (BRL-ejag) and long-name fitting (BRL-9k7q); this ticket only feeds
  them the marker.
- Lobby/supervisor/IPC/manifest changes; hold duration or rejoin limits.
- Notices for voluntary Leave Match (BRL-8y1f departures already remove the fighter; the
  projection simply disappears without a notice because it was never `HeldByBot`).


# Implementation (2026-11-18, at HEAD 2f796701)
## Decisions and adjustments to the plan
- `ParticipantControl { Player (default), HeldByBot { expires_at_tick } }` in `src/matchplay/model.rs`,
  re-exported from `matchplay`; new field `PublicParticipantState::control`. The server projection
  (`concealment::server::sync_public_participant_projections`) reads `Option<&crate::server::SeatHold>`
  (already `pub(crate)` re-exported at HEAD after BRL-jxfy); no schedule or seat_hold changes.
- Protocol 49 -> 50. Routing identity (`target/debug/brawler-server routing-identity`) identical
  before and after: network_protocol=4778953997791089217, protocol_registry_fingerprint=15212284317352318216,
  content_fingerprint=7772007507332977560. No test pinned 49, so a new
  `protocol::tests::public_participant_control_is_part_of_protocol_50` pins 50 and round-trips a held projection.
- Scoreboard (BRL-ejag, `hud/scoreboard.rs`): `CachedRosterEntry.held_until`; a held, connected row's
  status reads `BOT Ns` (hold seconds left, `SIMULATION_TICK_HZ`) in the dim Offline tone, replacing the
  status (boost suffix kept). Zombie team strip shows `BOT`. `DISCONNECTED`/`OFF` keep meaning "projection missing".
- Overhead (BRL-9k7q, `fighter_ui.rs`): a fixed-width 30 px BOT chip (11 px text, neutral slate) is the
  first child of the centered name row, `Display::None` unless held; held names are dimmed toward gray.
  `FighterOverheadNameUi.held` changes the fit budget to `PLAYER_NAME_MAX_WIDTH - 30 - 4` (98 px), so chip +
  gap + fitted name never exceed the 132 px name budget inside the 140 px plate; a held change refits.
  Never on the local (`Controlled`) fighter. Name/chip state is grouped in an `OverheadNames` SystemParam
  (clippy line limits).
- Feed (`hud/kill_feed.rs`): `KillFeedEntry { line: FeedLine::{Defeat, Seat{name, notice}} }`, same 4-row/4 s
  list; a seat row is name (relation color) + unbounded notice text, skull hidden. `SeatControlTransitions`
  resource, updated inside `collect_kill_feed`, emits exactly: Player->HeldByBot "disconnected - bot playing",
  HeldByBot->Player "is back", held projection gone while Active "left". First observation = baseline, never
  for the local player, reset on match id change.
- Acceptance 4 (Practice bot from start publishes Player) is covered by the server unit test, not a
  network scenario: tests/network has no Practice-bot match harness.
- Known limit: in a 1v1 the hold expiry forfeits the lone team in the same tick, so the client sees
  Completed and no "left" notice (spec: only while Active). "left" shows in team modes where the match continues.

## Verification
- `just lint` pass (with VS Code ripgrep on PATH); `just test` pass (network 132 tests, all suites);
  `just rejoin-e2e` pass ("Practice wipeout-1v1 rejoined the same running match ...").
- New tests: network `a_held_seat_is_published_as_bot_controlled_to_the_other_players`,
  `a_reclaimed_seat_is_published_as_player_again` (rejoined client's every observation of its own seat is
  Player), expiry assertion added to `an_expired_hold_removes_the_fighter_and_forfeits_a_lone_team`; server
  `concealment::tests::public_participant_projection_publishes_a_held_seat_as_bot_controlled` (held, reclaimed,
  Practice bot); client `seat_control_transitions_emit_only_the_three_notices`,
  `seat_notices_reach_the_feed_from_the_public_roster`, `team_strip_marks_a_bot_held_seat`,
  scoreboard `rows_use_readable_health_and_status_labels` (BOT 42s), overhead
  `bot_chip_marks_only_a_held_non_local_fighter`.

## Native desktop capture
- Own supervisor 127.0.0.1:5037, `--data-directory target/dwqx/run-kill/server-data`, protocol 50 roles;
  windowed release client 6001 (1280x720, `--render-report` keeps the windowed smoke alive,
  `--leave-match-cue` never written) and headless client 6002, SIGKILLed 5 s into Active. Seat held after
  the 10 s bound, expired 60 s later (1v1 forfeit). Logs in target/dwqx/run-kill/.
- target/dwqx/dwqx-notice-bot-playing.png: feed row "Brawler-1772 disconnected - bot playing" (enemy red name).
- target/dwqx/dwqx-bot-chip.png, dwqx-bot-chip-2.png: BOT chip before the dimmed enemy name, local plate unchanged.
- Not captured natively: scoreboard `BOT Ns` (needs a held Tab; covered by unit test), "is back" (needs a
  relaunch with the same account), "left" (1v1 forfeits). These remain for the owner's Pixel check.


# Closeout (2026-11-18)
Pixel not connected; device check left as playtest (steps above). Port-5000 test server restarted on protocol 50; rebuild/reinstall the APK before testing on device. 1v1 forfeit at hold expiry shows no "left" notice (match already Completed) - by the spec rule; reopen if a forfeit notice is wanted.
