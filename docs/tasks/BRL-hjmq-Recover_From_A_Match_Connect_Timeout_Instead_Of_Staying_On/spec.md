# Found
Pixel 8a, 2026-11-16 (BRL-2nsg cycle 6, HEAD 70a46541). During the lobby-to-match handoff, the client's match connect timed out after 10 s (`client connect failed. connection request timed out`, then `brawler client disconnected reason=Some("Transport error: ConnectionRequestTimedOut")`). The flow stayed in `MatchLoading` showing "Match loading / Connecting" for more than a minute, with no error and no retry. The match worker had already exited, because its formation failed without the phone.

# Outcome
When the match connection fails or times out during MatchLoading, the player gets a clear error with a way back (Retry or Dashboard), as the Dashboard connection-error overlay does.

# Plan
1. Find where `MatchLoading` waits for the match session (`packages/brawler-client/src/flow/`, `session/transitions`, `queue/`) and why a `ConnectionRequestTimedOut` disconnect on the match route isn't observed there.
2. Map that disconnect, and the match worker going away, to a flow transition that shows the connection error and returns to the lobby/Dashboard. Retry the handoff only if the match is still forming.
3. Add a flow test: MatchLoading + a match-route connect timeout leads to the error overlay and then Dashboard.

# Acceptance
- A flow test covers the timeout path. `just test` and `just lint` pass.
- Device: force a handoff failure (for example, stop the match worker during MatchLoading). The phone shows the error within about 10 s and gets back to the Dashboard.

Related: BRL-2nsg (the cause of the timeout itself).


# Result

## Root cause
`flow/observation.rs::observe_match_scope` only reacted to a lost match session (`MatchFailed`) and to the fresh lobby after it (`FreshLobbyReturn` / `LobbyReturnFailed`) when the flow was `Match`. For `MatchLoading` it returned `Complete(countdown?)` and nothing else. After the lobby-to-match handoff the client is in `RoutedClientPhase::Match` while still showing Match Loading. When the match connect timed out (`ConnectionRequestTimedOut`), `observe_client_lifecycle` correctly marked the match session `Disconnected`, but no flow observation saw it. Nothing requested a return to the lobby, so the screen stayed on "Match loading / Connecting" indefinitely. A lobby loss before the handoff during MatchLoading was also ignored, because MatchLoading is not one of the `LOBBY_FLOWS`.

## Fix (reuses the existing Match-flow recovery; no new states)
- `observe_match_scope` now treats MatchLoading like Match for a lost match session (`MatchFailed`, which requests the fresh lobby) and for a rejoin that has been accepted. It counts a lobby as a return only while a match loss is pending (`MatchFailureNotice`, now read by `observe_session`). So the lobby session before the handoff is never treated as a return, and while that lobby is fetching a rejoin grant the loading screen stays up. A lobby lost before the handoff during MatchLoading is reported as `UnexpectedLoss`, which goes to Server Select with Retry.
- `FreshLobbyReturn` from MatchLoading shows the Dashboard connection error "Could not connect to the match, so it did not start. You are back in the lobby." with Back (`MATCH_CONNECT_FAILED`). A failed fresh lobby shows the existing recoverable error (Retry / Change Server).
- Retry decision: the only server-side retry is the existing automatic rejoin (BRL-yyeg). It applies only to Active/Completed held seats. A match that is still forming cannot be re-entered, because a session lost in loading removes the fighter and formation fails. So there is no extra Retry button: the client takes the rejoin if the lobby announces one, and otherwise goes to the Dashboard with the explanation.
- Docs: `docs/13-player-ux.md` (flow diagram and error list).

## Verification
- New tests: `flow::tests::match_loading_connect_timeout_shows_the_error_and_returns_to_the_dashboard` (real observation path: the lobby before the handoff stays on MatchLoading; a `Disconnected` match session leads to `AwaitingMatchUnlink`; the fresh lobby leads to Dashboard with the error overlay; Dismiss clears it) and `flow::observation::tests::match_loading_observes_a_lost_match_session_and_the_lobby_after_it`.
- `just lint`: pass. `just test`: pass (2567 passed, 0 failed). `just rejoin-e2e`: pass (rejoined the same running match). `just practice-e2e`: pass. Logs are in `target/hjmq/`.
- Not done: no real-process check of the product shell. Headless automation clients do not run `ClientFlow` (`presents_product_shell` is false, and they use `enforce_routed_timeout` instead). Driving MatchLoading in a windowed client would need a new automation route.

## Pixel 8a playtest
Run `just server` with `BRAWLER_ROUTED_BIND=0.0.0.0:5000`, then `just android-build <host:5000>` and `just android-install`. Start Practice. As soon as Match Loading shows "Connecting", stop the match worker by its exact PID (the newest `brawler-server` child of the supervisor).
Expect: within about 10 s (the routed connect timeout) plus the fresh-lobby round trip, the phone leaves Match Loading for the Dashboard with "Could not connect to the match…". Back dismisses it, and Play/Practice work again. If the server is also unreachable, expect Server Select with Retry / Change Server instead.
