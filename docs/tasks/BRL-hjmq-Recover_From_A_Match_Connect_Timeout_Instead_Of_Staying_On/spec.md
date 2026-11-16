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
