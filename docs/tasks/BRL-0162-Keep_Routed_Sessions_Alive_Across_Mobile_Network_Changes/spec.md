# Outcome

Mobile network changes cause a short reconnect or a clear message, not a broken session.

# Context

Lightyear UDP identifies peers by remote socket address, so a Wi-Fi to cellular switch or NAT rebinding looks like a new peer and the old link times out. Some networks block UDP. Lightyear offers WebSocket/WebTransport transports.

# Scope

- Detect an address change or connection loss, then automatically reconnect through routed admission and rejoin the same match while it is still running.
- Bounded retries with a clear exit to the Dashboard.
- Evaluate and, if justified, add a WebSocket transport fallback.
- Handle app suspension longer than the server timeout.

# Out of scope

Server-side matchmaking changes beyond rejoin support.

# Acceptance criteria

- A Wi-Fi to cellular switch mid-match rejoins the same match within an agreed bound, or exits cleanly with a message.
- On a UDP-blocked network the client either connects through the fallback or shows an actionable error.
- Server authority and admission rules are unchanged.

# Verification

- A routed-process test for rejoin.
- A device test.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
