# Origin

Owner request, 2026-11-18: "make maker map playable, need to think how a player may upload it to a server that he doesn't own and how it's put in rotation".

# Current state

- Map Maker drafts are local client documents (BRL-6vjz). They can be test-played locally, but there is no path to a routed server's rotation.
- Built-in maps come from `content/maps/` and `config/server/game-types.ron`, and they are part of the gameplay content fingerprint (docs/08). A routed match only admits clients whose fingerprint matches.
- The server is authoritative (AGENTS.md invariants). Maps are bounded recipes over server-known assets and supported modes; they never carry executable rules.

# Open design questions (answer before implementing; ask the owner via Ticket questions)

1. **Submission:** how does a player upload a map to a server they don't own? Options include an in-client "Submit to this server" action over the existing routed connection, with size and rate limits per account.
2. **Trust and moderation:** who approves a submitted map? Options: automatic validation only; an operator queue in the Balance Lab or a profile_admin command; community voting. What happens to offensive names?
3. **Validation:** the server must re-validate the recipe with the same authoritative validator (bounds, asset IDs, spawns, mode objectives). Clients never send geometry the server can't check.
4. **Fingerprint and compatibility:** user maps must not change the global content fingerprint or split compatible clients. Likely shape: a separate, versioned user-map catalog distributed by the server, identified by content hash, with the recipe sent to clients at match load.
5. **Rotation:** how are accepted maps scheduled? Options: an operator-curated rotation list per game type, a weighted mix of built-in and community maps, a "community" playlist, or private lobbies only at first.
6. **Ownership and attribution:** the map shows its author's display name; the author can withdraw it; it persists in the server's data directory with the profiles.
7. **Abuse limits:** maximum maps per account, recipe size caps, cool-downs, and how a server operator removes a map.

# Suggested slices (after the decisions)

1. The server accepts and validates a submitted recipe and stores it as pending.
2. An operator approves it (CLI or Balance Lab).
3. Approved maps are distributed to clients at match load and enter a rotation for one game type.
4. Player-facing status: submitted, approved or rejected, and the author credit in match loading and results.

# Acceptance (for the eventual implementation)

- A map made on one client is submitted to a routed server, approved, and played in a real routed match by other clients, with the server authoritative throughout.
- The content fingerprint and protocol rules in docs/08 still hold; docs/04 and docs/14 describe the new flow.
- Network tests cover submission, validation rejection, and distribution at match load.
