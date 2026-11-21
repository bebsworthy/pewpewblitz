# Outcome
A client cannot act as another player's account, either by replaying a token or by reading another player's traffic, and a leaked database or backup does not reveal account credentials.

# Found (2026-11-21)
- `AccountId` is 128 bits from `getrandom` (`src/profiles/model.rs:27`), generated per logical server on the client (`packages/brawler-client/src/connection_persistence.rs:268`). It is stored in the client's connections file and treated by the lobby as a bearer credential (`src/server/lobby/account_takeover.rs` header).
- On the wire it appears only in `LobbyHello` (`src/protocol.rs:220`) and in the owner's own `ProfileSnapshot` (`src/profiles/model.rs:126`); no other player receives it (checked serialized types, 2026-11-21).
- The actual exposure was transport (see BRL-z277). After BRL-z277 the secret is confidential in transit.
- The database keys every account row by the raw secret (`src/profiles/storage.rs`, `published_maps.rs`), so a database or backup leak is a credential leak.
- Any unknown secret implicitly creates an account.

# Decisions
- No account recovery at launch (owner, q-42qkqj0). Because the launch is public, the client tells players that reinstalling or clearing app data starts a new account (settings or account text), and a backlog recovery ticket is filed when this ticket starts.
- **Token binding:** the token service puts the account secret in Netcode `user_data` (inside the server-encrypted private part). The lobby takes the account from `TokenUserData` and rejects a hello whose `account_id` differs (or drops the hello field; choose at implementation and record).
- **Storage at rest (owner-approved 2026-11-21):** the server derives the stored account key as `SHA-256(domain tag ‖ secret)` and never stores the secret. The client's `ProfileSnapshot.account_id` stays the client-side secret, or is removed from the wire if unused. With no real players yet, this ships before the first public deploy without a data migration, under the BRL-kdza migration policy.
- Android storage stays app-private. Moving to the Keystore is deferred; it stops no attack that app-private storage allows on a non-rooted device.

# Scope
1. Lobby reads the account from `TokenUserData`; reject on mismatch; network test for a mismatched hello.
2. Hashed account keys in profile storage and published maps; `brawler-profile-admin` takes and prints the derived key, never the secret.
3. Player-facing "new account on reinstall" text.
4. Account creation counted against the per-source and per-token-service limits defined in BRL-fygr (cross-link only; limits are set there).

# Acceptance
- A network test: a client with a valid token for account A sending a hello for account B is rejected with `InvalidAccount`.
- A storage test: the database contains no column equal to the client secret.
- Relaunch takeover (`account_takeover.rs`), `just rejoin-e2e` and `just network-change-e2e` keep the same account.

# Verification
`just lint`, `just test`, the e2e runs above. Blocked by BRL-z277.
