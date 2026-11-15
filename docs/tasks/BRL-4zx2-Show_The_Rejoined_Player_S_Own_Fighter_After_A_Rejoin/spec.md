# Found
Device check 2026-11-15 (Pixel 8a), HEAD b97a8888 (protocol 49), server `just server` on 0.0.0.0:5000, during the BRL-8y1f involuntary-loss check. Reproduced 3 of 3 successful rejoins.

# Repro
1. Phone (player Brawler-42DD) queues Wipeout 1v1. A headless opponent (`brawler-client --auto-connect --headless --product-match-smoke-1v1 --product-game-type wipeout-1v1 --client-id 510N`) queues too. The headless client exits about 13 s into the match, so its seat is held by a server bot (hold 60 s).
2. In the match: `adb shell am force-stop com.pewpewblitz.client`, wait about 13-15 s, relaunch, then CONNECT.
3. The lobby sends a rejoin and the server logs "seat reclaimed by a rejoined session player_id=<phone's id>". The phone logs "rejoined its running match player_id=<same id>; network_entity_id = 2" with the same roster as before.

Expected: the same fighter, with its own name plate, the HUD scores from the phone's team, and the camera on the phone's fighter.
Actual: the camera-centred fighter with the green "self" ring is labelled with the opponent's name (Brawler-13EF / 13F0 / 13F2, the headless client's name). The kill feed shows the opponent's name in green killing Brawler-42DD in red. The HUD blue score is the opponent's score: blue 1 / red 0 in the match, but Results said Your team 0, Opponents 1. In r5-pre both fighters have green name plates.
Before the force-stop the same match showed Brawler-42DD correctly (f2-before, r3-before).

# Notes
- In all three repros the opponent's seat was held by a server bot at rejoin time. In the BRL-yyeg device check the phone rejoined with its own name plate, so the held opponent or the entity order may matter: the phone was network_entity_id 2 every time.
- Not confirmed whether touch input moves the phone's own fighter or the other one: the match ended by forfeit before the joystick check finished.

# Acceptance
- A rejoined client presents its own fighter (local-player marker, name plate, camera, team-relative HUD and kill feed) in a test that rejoins while the other seat is bot-held.
- Pixel 8a: after force-stop and rejoin, the name plate and HUD match the Results screen.

Evidence (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/vc): f2-before.jpg, f2-rejoin.jpg, f2-label.png, l2-after.jpg (Results), r3-before.jpg, r3-b.jpg, r3-c.jpg, r3-e.jpg (Results), r5-c.png, r5-pre.jpg, r5-post.jpg; f2-logcat.txt, r3-logcat.txt, r5-logcat.txt; server.log (17:27:38, 17:29:47, 17:36:22).

# Root cause (2026-11-15)
Every client "who am I" lookup (HUD readiness/score owner, camera rig, self ring and name plate in `fighter_ui.rs`, kill-feed colours, combat audio, input `InputMarker` via `add_controlled_input_marker`) reads the first fighter with Lightyear's `Controlled` marker. Results instead reads the admitted `player_id` from `ClientJoinStatus`, which is why Results disagreed with the HUD.

Lightyear 0.29 materialises `Controlled` on a receiver from the replicated sender marker `ControlledSend`, which `ControlledBy` pulls in as a required component. Per-link visibility of `ControlledSend` is only managed for entities that *have* `ControlledBy`: `ControlledBy::on_insert` hides it from every non-owner link, and `handle_new_client_visibility` hides it from a newly connected link only for entities whose `ControlledBy.owner` is another link.

`hold_released_seats` (`src/server/seat_hold.rs`, BRL-yyeg) removed `ControlledBy` (and the input buffer) from the held fighter but not `ControlledSend`. A held fighter therefore kept a `ControlledSend` that no visibility rule hid, so any session that connected during the hold, including the rejoining phone, received the bot-held opponent as `Controlled` too. The phone then had two `Controlled` fighters; first-match lookups picked the opponent (it spawned first, network_entity_id 1 vs the phone's 2), giving the opponent's name with the self ring, the opponent's team as "blue" in the HUD and kill feed, and an `InputMarker` on both fighters.

Server input routing was already correct: `record_unauthorized_input_targets`/`authorize_controlled_targets` accept input only for a fighter whose `ControlledBy.owner` is the sending link, and a held fighter has none, so input aimed at the opponent was rejected (counted as ownership rejections) and could never drive it. The phone's own fighter was correctly re-owned by `reclaim_held_seat`.

Not bot-held opponent: its fighter keeps `ControlledBy { owner: <its link> }`, so the new link's visibility setup hides its `ControlledSend`; the rejoining client sees only its own fighter as `Controlled`. That explains why the BRL-yyeg device check (opponent still connected) looked right. Covered by the strengthened existing test below.

# Fix
`src/server/seat_hold.rs`: the hold removes `(ControlledBy, ControlledSend, NativeBuffer<FighterInput>)`. Reclaiming inserts `ControlledBy`, which re-adds `ControlledSend` and makes it visible to the rebound link only. No client change; no wire, encoding, manifest or control-IPC change, so no protocol or control-version bump. Durable note added to `docs/14-multiplayer-server-architecture.md` (Rejoining a running match).

Routing identity (`target/debug/brawler-server routing-identity`), identical before and after: network_protocol=4778953997791089217, protocol_registry_fingerprint=15212284317352318216, content_fingerprint=7772007507332977560; SUPPORTED_PROTOCOL_VERSION 49, CONTROL_VERSION_CURRENT 7.

# Verification
- New `tests/network/match_rejoin.rs::a_rejoin_while_the_other_seat_is_bot_held_controls_only_its_own_fighter`: the opponent's session is lost first (bot-held), then the player's; the player rebinds and reclaims; asserts the admitted player id, exactly one `Controlled` fighter (its own), `InputMarker` only on its own fighter, its input moves its own fighter with zero server ownership rejections, the opponent stays bot-held; then the opponent reclaims and each client owns exactly its own fighter. Before the fix it failed with `left: [7, 10] right: [7]`; it passes after.
- Strengthened `a_session_lost_beyond_the_timeout_rejoins_the_same_bot_held_fighter` (opponent still connected) to assert exactly `[own player]` is `Controlled` (helpers `controlled_player_ids`, `admitted_player_id` in `tests/network/harness.rs`).
- Pixel 8a (45171JEKB03025), server rebuilt on the fix (`just server`, 0.0.0.0:5000), app reinstalled with `scripts/android.sh install 192.168.31.211:5000`, phone vs headless `--product-match-smoke-1v1` Wipeout 1v1; the headless client exits about 13 s in so its seat is bot-held; force-stop, relaunch about 15 s later, CONNECT. 2 of 2 rejoins correct (`seat reclaimed by a rejoined session`, phone network_entity_id 2 as in the repro):
  - Run a: camera-centred fighter with the green self ring labelled Brawler-42DD; opponent Brawler-1389 red name plate and red ring; kill feed "Brawler-1389 (red) killed Brawler-42DD (green)"; HUD blue 2 / red 3 equals Results "Your team 2, Opponents 3".
  - Run b: own name plate and ring; holding the joystick up then left moved the phone's own camera-centred fighter (scene scrolled, fighter reached the north wall); HUD blue 0 / red 1 equals Results "Your team 0, Opponents 1".
  - Evidence in /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/vc/: a1-match.png, a1-rejoin.png, a1-move0.png, a1-move1.png (kill feed), b-rejoin.png, b-up.png, b-left.png, b-results.png, b-logcat.txt, server-4zx2.log.

# Learn from errors
- Mistake: BRL-yyeg detached control by removing only the relationship component (`ControlledBy`) and assumed the library's derived replicated marker went with it. Lightyear's required component (`ControlledSend`) stays behind, and its per-link visibility is only maintained while `ControlledBy` exists.
- Why it was missed: the BRL-yyeg separate-App test and device check rejoined with the other seat still connected, and the harness helper `controlled_player_id` returned the first `Controlled` fighter, so a second `Controlled` fighter was invisible to the assertion.
- Prevention: when removing a Lightyear relationship component, also remove the components it `#[require]`s that replicate; assert "exactly one `Controlled` fighter, equal to the admitted player" (not "first") in ownership tests; cover rejoin with every other seat both connected and bot-held.

- Canonical: `just check`, `just lint`, full `just test` (all suites ok, 0 failures), `just e2e 2` ("1 exact 1v1 roster(s) reached Active") and `just rejoin-e2e` ("rejoined the same running match after an outage beyond the session bound") pass on the final tree.
