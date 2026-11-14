# Outcome
A failed lobby return after a match ends in the existing recoverable error panel (Retry / Change
Server), never a dead screen.

# Evidence (BRL-0196 device walk, Pixel 8a over Wi-Fi, 2026-11-13)
- After a Practice match the client logged "starting fresh lobby session generation=4", then
  "client connect failed. connection request timed out" and "brawler client disconnected", but the
  screen stayed on MATCH COMPLETE / RETURNING TO LOBBY indefinitely (several minutes); only a
  force-stop recovered. The same phone's first connect after launch also timed out once while the
  Mac server was healthy, so first-packet loss on Wi-Fi makes this path reachable.

# Scope
- When the post-match lobby session fails (timeout or disconnect) while the flow is presenting match
  completion, commit the same connection-lost error used elsewhere, with Retry returning to the lobby.
- Consider one automatic retry of the lobby connect before showing the panel.

# Acceptance criteria
- A focused flow test injects a lobby connect failure during match completion and observes the error
  overlay; the Pixel 8a recovers by Retry without restarting the app.


# Fixed by BRL-0162 (2026-11-14, uncommitted; status left for the owner)
- Cause: when the fresh lobby generation after a match failed while the flow still showed the
  match, `observe_session` had no rule for it, so the screen stayed on MATCH COMPLETE. A second
  defect on the same path: a match session lost to a Netcode timeout was never unlinked
  (`drive_routed_transition`), which left a dark match screen.
- Fix: `LobbyReturn` state with `SessionObservation::LobbyReturnFailed` commits the Connection Lost
  panel (Retry / Change Server). The copy is "Could not reconnect to the lobby after the match.
  Check your network, then retry." after a completed match, and "The connection to the match was
  lost and the lobby could not be reached…" after a lost match. `drive_routed_transition` now
  unlinks any disconnected session during a transition.
- Evidence: the flow test
  `failed_lobby_return_during_match_presentation_shows_recoverable_connection_error` drives the
  real observation path; the observation unit test and the transition unit test cover the rest.
  On the Pixel 8a, a Wi-Fi-off mid-match reached the panel, and after the second Retry it reached
  Dashboard without restarting the app. The scenario was a lost match, not a completed one.
- Not done: no automatic retry before the panel; the first Retry can fail while Android settles
  the network. A device check of the completed-match variant was not run.
