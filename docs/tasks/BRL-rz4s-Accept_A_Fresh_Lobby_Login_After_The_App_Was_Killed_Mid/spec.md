# Outcome
A relaunched client is not locked out for up to the 10 s routed session bound by its own previous, now-dead lobby session.

# Evidence (BRL-g57a device walk, Pixel 8a, 2026-11-14)
After am force-stop and an immediate relaunch, Connect was rejected with LobbyJoinRejection AccountInUse; it succeeded once the old lobby session timed out (about 10 s, the BRL-g57a session bound).

# Scope
- Decide the lobby policy for a second admission of the same account while the first session is silent (for example replace the older session when the new one authenticates with the same account binding), keeping server authority and admission rules explicit.
- Shell copy for AccountInUse if a wait remains necessary.

# Acceptance criteria
- A focused lobby test covers the replace-or-reject decision; a device relaunch within 10 s reaches Dashboard or shows an actionable message.



# Research and design (2026-11-14, before implementation)

## Findings
- **The account credential is the `AccountId`, not the player name.** The client draws a random
  128-bit `AccountId` from OS entropy once per logical server (`connection_persistence.rs`
  `account_for_server`) and presents it in `LobbyHello`. The display name is only a proposal that
  the lobby suffixes; it never selects a profile. `AccountId` is never sent to other players, is
  not in any match manifest (`docs/14`), and is not replicated. Knowing a player name therefore
  cannot select or displace an account. (Per `docs/02`/`docs/13`, the `AccountId` is a development
  bearer seam, not a production credential; anyone holding it can already load the profile
  whenever the owner is offline.)
- **Where the lockout comes from.** `ProfileAuthority::begin_load` rejects any second load while
  `active_accounts` (or `pending_accounts`) holds the account; the entry is released only by
  `remove_client`, which runs when the lobby's Netcode connection for the old client ends. After
  `am force-stop` no disconnect packet is sent, so the old connection ends only at its Netcode
  token timeout (10 s, BRL-g57a). The relaunched app uses a fresh random Netcode client ID and a new
  UDP source port, so its Netcode connection and lobby route are new and connect fine; only the
  profile admission rejects it.
- **Liveness evidence available on the server.** A live client sends Netcode keep-alives at 10 Hz
  (`lightyear_netcode` `PACKET_SEND_RATE_SEC`) plus pings and messages. Netcode's own
  `last_receive_time` is private, and Lightyear 0.29 exposes no public per-client server-side kick
  or last-receive accessor. Every inbound datagram for a lobby peer does pass through its `Link`
  receive buffer between `LinkSystems::Receive` and `ConnectionSystems::Receive`, for both the
  routed worker transport and Crossbeam, so the lobby can record "last heard" per link without any
  transport or wire change.
- **Matches in progress.** At handoff the client closes its lobby connection before connecting to
  the match (`docs/14` steps 5-7), so the lobby already holds no profile session for a player in a
  match. A kill mid-match therefore does not block the relaunched lobby login; the old match keeps
  the old fighter until the match session times out at the 10 s bound and handles it as a lost
  participant. The relaunched app reaches Dashboard and gets no second fighter in that match. A new
  match it queues for is a separate match. Rejoining the running match is BRL-yyeg (deferred).
- The supervisor is not involved: lobby authentication facts carry route/peer/session/Netcode IDs
  and no account. The old lobby route closes through the existing peer-close path when the old
  Netcode connection is finally despawned.

## Decisions
1. **Policy: replace a silent session of the same account.** A new lobby hello whose `AccountId`
   is held by another lobby session (accepted or still loading) replaces it only when the holder
   has sent nothing for `STALE_LOBBY_SESSION_SILENCE` = **2 s** (20 missed keep-alives).
   Otherwise the hello waits: it is decided as soon as the holder's silence reaches 2 s
   (replace), or rejected with `AccountInUse` once it has waited 2 s while the holder kept
   sending. A live duplicate (two running copies with the same account) still gets
   `AccountInUse`. Only the `AccountId` (the device-held secret) can trigger replacement, so a
   name alone cannot. The outcome does not depend on how fast the app relaunches.
2. **Replacement is an explicit lobby transaction.** The lobby removes the old profile session
   from `ProfileAuthority`, marks the old connection `Disconnected`, and records it as a lobby
   session loss, so the existing `cleanup_disconnected_sessions` cancels its queue ticket,
   reservation, or Practice formation. The other participants see `ParticipantLost`, as if the
   old client had disconnected. Then the new hello's profile load is submitted. The profile store's
   single FIFO executor orders the new load after any mutation the old session had in flight.
   The old Netcode connection is left to time out, as in the existing abuse-disconnect path,
   because Lightyear 0.29 exposes no public server-side kick.
3. **No wire, IPC, manifest, or admission-rule change.** `LobbyHello`, `LobbyJoinRejection`, and
   the control IPC are unchanged. The waiting hello uses the client's existing connect-attempt
   bound (10 s for product lobby attempts).
4. **Copy.** A remaining `AccountInUse` rejection now says that the account is already connected
   from another running copy of the game, with Retry and Back. It no longer shows the generic
   "rejected compatibility" text.
5. **Test seam.** `ClientNetworkConfig` gains an optional deterministic account ID for headless
   and test clients. Today a test client's account equals its Netcode client ID, so a relaunch
   (new Netcode ID, same account) cannot be expressed otherwise. The product shell continues to
   use the persisted per-server account.

## Verification plan
- Unit: the replace/wait/reject decision; `ProfileAuthority` account-holder lookup; AccountInUse
  copy.
- Separate-App network scenarios (`tests/network/`, product lobby harness): (a) a suspended
  (killed) client's session is replaced by a fresh client with the same account, including its
  queue ticket being removed; (b) a live client with the same account keeps its session and the
  newcomer is rejected with `AccountInUse`.
- `just lint`, `just test`, `just e2e 2`, `just network-change-e2e`; Pixel 8a kill and immediate
  relaunch from Dashboard and mid-Practice.



# Implementation (2026-11-14)
- `src/server/lobby/account_takeover.rs` (new): `STALE_LOBBY_SESSION_SILENCE` (2 s);
  `LobbyLinkActivity` stamped by `record_lobby_link_activity` (PreUpdate, after
  `LinkSystems::Receive`, before `ConnectionSystems::Receive`); `AccountTakeovers`, the deferred
  hellos (at most one per Netcode connection); `resolve_account_takeovers` (chained after
  `lobby_receive_hellos` in `AuthenticateLobbyHellos`), which applies the pure
  `decide_takeover` (Replace / Wait / Reject); `replace_holder`, which removes the holder from
  `ProfileAuthority`, detaches it from `LobbyState` at once, drops a still-loading admission,
  records the lobby-session loss, and marks the old connection `Disconnected`.
- `src/server/lobby/profile_transactions.rs`: a hello whose account has a holder is deferred
  instead of rejected; `begin_admission` is shared by both paths.
- `src/profiles/authority.rs`: `ProfileAuthority::account_holder`.
- `src/client/flow/reducer.rs`: AccountInUse copy "This account is already connected from another
  running copy of the game. Close it, then retry." with Retry / Back.
- `src/config.rs`, `src/client/session/admission.rs`: `ClientNetworkConfig::deterministic_account_id`
  (headless/test account; `None` keeps `client_id`).
- Tests: decision unit tests (4); `account_holder` assertions in the existing profile authority
  test; AccountInUse copy/actions in the flow test; `tests/network/lobby_takeover.rs` (product-lobby
  harness, new `Harness::add_product_lobby_client` and `suspend_client`): a killed session with a
  queue ticket is replaced between 1.5 s and 2.5 s (before the harness's 3 s Netcode timeout), the
  ticket is removed and the relaunch gets the same saved brawler; a live session keeps its account
  and the duplicate is rejected only after waiting 2 s; a different account is never replaced.
- Docs: `docs/14` "Relaunch after the app was killed"; `docs/13` error contract.
- No wire, manifest, control-IPC, or capability change.

# Verification (2026-11-14)
Evidence: agent scratchpad `brl0208/` (logs, logcat dumps, screenshots, kill timelines), not kept
in the repo.
- `just lint` (ripgrep 15.0.0 first on PATH): **pass** (final run after every change).
- `just test`: **pass**, 23 suites `test result: ok`. The first run had 2 failures:
  `lifecycle::connected_client_without_hello_times_out_without_owned_entities` was my harness
  bug (a client pushed directly into `harness.clients` had no suspension entry; fixed), and
  `prediction::dash_converges_without_snapping` failed on "no visible teleport". The prediction
  test is unrelated, passed 3/3 alone and in both later full runs, and is filed as BRL-thwh.
- `just e2e 2`: **pass** (1 exact 1v1 roster reached Active), final code.
- `just network-change-e2e`: **pass** (kept its match session across outage and source-port
  change), final code.
- **Pixel 8a device** (Wi-Fi to 192.168.31.47:5001, `am force-stop` then an immediate `am start`,
  then Connect about 2 s later; the app opens on Select Server):
  1. Dashboard kill, run 1: the server logged "lobby replaced a silent session of the same account
     … silent_ms=2918" and the relaunch reached Dashboard about 30 ms after its hello. **Bug
     found:** the name showed "Brawler-42DD #2". The new profile load completed in the same frame,
     before `cleanup_disconnected_sessions` released the old accepted name. Fixed by detaching the
     replaced session from `LobbyState` inside the replacement (screenshot `03-…`).
  2. Dashboard kill, run 2 (fixed server): replaced with silent_ms=2417; Dashboard shows
     "Brawler-42DD" and the same Brawler 2 (`05-…`).
  3. Mid-Practice kill (Feature Yard Wipeout 2v2, match running): the lobby session had already
     closed at handoff, so the relaunch was accepted directly (no replacement needed). Dashboard
     was reached about 4 s after the kill with the same profile (`08-…`). The old match worker was
     reaped about 10.1 s after the kill (match session timeout). No second fighter joined that
     match.
- Before this change, the same relaunch was rejected with AccountInUse for about 10 s
  (BRL-g57a device walk).

# Playtest request
Server: `BRAWLER_ROUTED_BIND=0.0.0.0:5001 just server`; phone: `just android-install <MAC_IP>:5001`.
1. On Dashboard, swipe the app away (or force-stop it), reopen it at once, and tap Connect.
   Expect Dashboard within about 2 s of the kill (at most a 2 s wait), with your name unchanged
   (no "#2").
2. Do the same mid-Practice. Expect Dashboard straight away. The abandoned match ends on its own.
3. If you have two devices or a desktop client sharing one connections file, connect both with
   the same account. Expect the second to show "This account is already connected from another
   running copy of the game. Close it, then retry." after about 2 s.
Please report whether the 2 s silence bound feels right.

# Feedback disposition
- No owner feedback yet. The 2 s bound and the "latest login replaces only a silent session"
  policy await the owner's confirmation.

# Deferred
- BRL-thwh (todo, parent BRL-q5c4, related BRL-rz4s): flaky
  `prediction::dash_converges_without_snapping` in the full network suite.
- Not filed: an explicit server-side Netcode kick of the replaced connection. Lightyear 0.29 has
  no public API for it. The replaced connection is ignored and times out within 10 s, so the only
  cost is up to 10 s of keep-alives to a dead address.
- Not filed: rejoining the abandoned match after relaunch, which is already BRL-yyeg.

# Learn-from-errors
- **Mistake:** the first device run showed the relaunched player as "Brawler-42DD #2".
  **Cause:** I relied on the existing loss cleanup (`cleanup_disconnected_sessions`), which runs
  later in the frame than profile-load completion. The harness test passed only because its
  storage load never completed in the same frame. **Prevention:** when one transaction hands a
  resource (account, name, slot) from an old owner to a new one, release everything the new owner
  can collide with inside that transaction, not in a later cleanup stage. Then check the schedule
  order between "release" and "next possible acquire".
- **Mistake:** adding per-client harness state (`suspended_clients`) indexed alongside
  `clients` broke a test that pushes into `clients` directly. **Prevention:** derive optional
  per-client harness flags with `.get(index)` defaults, or extend every construction path, and run
  the whole network suite, not only the new tests.
