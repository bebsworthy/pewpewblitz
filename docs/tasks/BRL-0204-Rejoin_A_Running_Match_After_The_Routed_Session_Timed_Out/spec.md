# Outcome
A lost match session can be re-established while the match still runs.

# Context (from BRL-0162, 2026-11-14)
BRL-0162 keeps the same Netcode session alive through gaps up to the 10 s routed session bound (capability idle revocation). Beyond that the client returns to a fresh lobby and Dashboard. Rejoin is not supported today: the match worker rejects hellos once the match left Waiting (MatchInProgress); admission allows one live connection per manifest participant; the capability/route is revoked on idle or PeerClose; the manifest binds each participant to one PeerId.

# Scope
- Lobby recognises an account whose match is still running and requests a fresh capability for that participant.
- Supervisor control IPC to mint a capability for an existing worker route/peer (or a re-bindable peer), with expiry and rate limits.
- Match worker re-admits an in-progress manifest participant and re-attaches control of the existing fighter (or a defined respawn).
- Client: on match-session loss, prefer the rejoin grant over Dashboard.

# Acceptance criteria
- A routed-process test drops a client longer than 10 s mid-match and rejoins the same match and fighter.
- Server authority and manifest admission remain the only identity checks.

# Notes
Needs control-IPC and manifest contract changes; validate the architecture decision first (docs/14).
