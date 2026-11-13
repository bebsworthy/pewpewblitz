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
