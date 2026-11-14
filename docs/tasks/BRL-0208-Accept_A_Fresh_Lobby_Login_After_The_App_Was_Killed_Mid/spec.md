# Outcome
A relaunched client is not locked out for up to the 10 s routed session bound by its own previous, now-dead lobby session.

# Evidence (BRL-0162 device walk, Pixel 8a, 2026-11-14)
After am force-stop and an immediate relaunch, Connect was rejected with LobbyJoinRejection AccountInUse; it succeeded once the old lobby session timed out (about 10 s, the BRL-0162 session bound).

# Scope
- Decide the lobby policy for a second admission of the same account while the first session is silent (for example replace the older session when the new one authenticates with the same account binding), keeping server authority and admission rules explicit.
- Shell copy for AccountInUse if a wait remains necessary.

# Acceptance criteria
- A focused lobby test covers the replace-or-reject decision; a device relaunch within 10 s reaches Dashboard or shows an actionable message.
