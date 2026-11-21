# Outcome
Only the server can mint Netcode connect tokens, every session's keys reach the client confidentially, and no client build contains the server key.

# Found (2026-11-20/21)
- Every role uses `DEVELOPMENT_PRIVATE_KEY = [0x42; 32]` (`src/protocol.rs:50`). Clients self-sign with `Authentication::Manual` (`packages/brawler-client/src/session/connection.rs:89`, `src/lib.rs:880`) and `token_expire_secs: -1`; servers use `.with_key` (`src/server/mod.rs:447`, `:1496`, `src/server/worker.rs:480`).
- Lightyear 0.29: the token's public part carries the session keys (`lightyear_netcode/src/token.rs:307`). The private part, encrypted with the server key, carries `client_id`, timeout, the server address list, the keys and 256 bytes of `user_data` (`token.rs:135`, `lib.rs:121`). Lightyear expects an HTTPS backend sharing the protocol id and key (`auth.rs`). The server exposes `user_data` as `TokenUserData` (`server_plugin.rs:28`, `:329`) and rejects reused tokens (`server.rs:710`).
- So with the public key, anyone can mint tokens and a passive observer can decrypt any session, including `LobbyHello.account_id` and each `MatchRouteGrant` route capability.
- Each lobby connect, match handoff and rejoin is a new Netcode client with a random `client_id` (`config::random_nonzero_client_id`, `session/routing.rs:310`). The lobby binds `netcode_client_id` into the match manifest, and match admission checks it with the route peer (`src/server/admission.rs:161`). The token timeout is the routed keep-alive bound (`ROUTED_SESSION_TIMEOUT` = `CAPABILITY_IDLE_MILLIS`).
- `tiny_http` is already a dependency, but only behind `balance-lab` (`src/server/balance_lab/http.rs`).

# Decisions
- **First lobby token comes from an HTTPS token endpoint** (owner, q-1x2b2tp, 2026-11-21). This amends the single-UDP-endpoint statement in docs/14: game traffic stays on one UDP port, and token issuance adds HTTPS on TCP 443.
- **Shape (owner-approved 2026-11-21):**
  - A small stateless `brawler-token-service` binary, a `server`-feature `brawler` bin that holds only the key and config.
  - It speaks plain HTTP on a private container network. A TLS-terminating reverse proxy (Caddy, automatic Let's Encrypt) in the same compose file publishes 443 under the BRL-3he3 DNS name.
  - It has no database access; account binding uses token `user_data` (BRL-4xjh).
  - Why: no TLS or certificate code in Brawler, the supervisor stays free of HTTP and is not a new failure point, and the service restarts on its own.
- **Match and rejoin tokens are minted by the lobby** (it holds the key) and sent inside the now-confidential lobby session alongside `MatchRouteGrant`. The client connects to the match with `Authentication::Token`.
- **Network-change continuity** keeps the existing Netcode session and needs no token. Relaunch and lobby reconnect fetch a fresh token.
- **Key:** an operator-provided 32-byte secret (a container secret file), read by the supervisor and token service and passed to lobby and match workers through their private manifests. A release supervisor and token service refuse `DEVELOPMENT_PRIVATE_KEY`. Dev, e2e and `network-test` topologies keep it.
- **Token lifetime:** about 30 s expiry, and the timeout equals `CAPABILITY_IDLE_MILLIS` so keep-alive behavior doesn't change. Each token is single-use (Netcode already enforces this).
- **Client:** release builds accept only `https://` token URLs; dev and automation builds may use `http://` on loopback. Adds a client-only HTTPS client dependency (`ureq` with rustls and webpki roots, which builds for Android), to be recorded in the dependency review at implementation.

# Scope
1. A shared token-minting helper in the server role (protocol id, key, public UDP address, timeout, `user_data`), used by the token service and the lobby.
2. `brawler-token-service`: `POST /v1/lobby-token` with a body holding the account secret and client build fingerprints. It replies with token bytes or a compatibility rejection. Bounded request size, per-source rate limit, structured logs that never include the secret or token.
3. Lobby: mints match and rejoin tokens; `MatchRouteGrant` carries the token; the protocol version bumps.
4. Client: fetch the lobby token before the lobby connect, use `Authentication::Token` for lobby, match and rejoin, and show the existing connection-failure states when the token fetch fails.
5. Key loading and the release refusal in the supervisor, token service and worker manifests.
6. Dev topology: `scripts/dev.sh`, the e2e scripts and `just server` start the token service on loopback HTTP. The network harness keeps Manual auth under `network-test`.
7. Docs: the token flow in `docs/08-network-architecture.md`; a docs/14 amendment for the HTTPS issuance port; the key, proxy and certificate runbook in `docs/19-server-operations.md`.

# Non-goals
Account recovery; platform sign-in; storing accounts in the token service; moving gameplay off UDP.

# To verify during implementation
Whether the routed IPC link reports a server address to Netcode (`server.rs:675`). If it does, the token must list the public UDP address the client targets, and NAT or DNS changes need care.

# Acceptance
- No client binary contains the server key: a release-build check greps the binaries for the key bytes, and the release client has no `Authentication::Manual` path.
- A release supervisor or token service started with the development key exits with a clear error.
- A test proves that a self-minted token (development key) is refused by a server with an operator key.
- `just practice-e2e`, `just e2e 2`, `just rejoin-e2e`, `just network-change-e2e` and `just completed-rejoin-e2e` pass with server-issued tokens.
- Packets captured during `just e2e 2` cannot be decrypted with the development key (scripted check).

# Verification
`just lint`, `just test`, the e2e list above, and an Android connect through the DNS name and proxy once BRL-3he3 and BRL-jjwa exist (manual, recorded here).
