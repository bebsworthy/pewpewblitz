# Outcome
An operator can deploy a new server version without cutting matches in progress, and a player on an outdated client is told to update, with a direct way to do it, instead of getting a debug error.

# Found (2026-11-21)
## Compatibility gate
- The lobby rejects a hello whose `protocol_version` ≠ `SUPPORTED_PROTOCOL_VERSION` (55), whose `build_version` ≠ `VERSION` (`CARGO_PKG_VERSION`, exact string), or whose registry or content fingerprint differs (`src/server/lobby/mod.rs:422-434`). The match admission path does the same (`src/server/admission.rs`).
- **Exact `build_version` equality means every server version bump rejects every client**, even when the wire is compatible. Today the version is pinned at `0.1.0` (`Cargo.toml:3`), so in practice the fingerprints decide.

## What the player sees
- Any compatibility rejection falls through to a debug string: `"The server rejected compatibility: {rejection:?}"`, for example `ContentMismatch`, with only a Back action (`packages/brawler-client/src/flow/reducer.rs:765`).
- There's no update prompt and no store link. Android has one Java `startActivity` (local-network settings, `MainActivity.java:192`), which shows the JNI bridge pattern a store link would follow.

## Release versioning
- Android `versionCode 1` / `versionName '0.1.0'` are hardcoded (`packages/brawler-android/android/app/build.gradle:53`). Google Play rejects an upload whose `versionCode` doesn't increase, so every store release needs a new value.

## Shutdown and drain
- Graceful shutdown stops every worker within 5 s (`graceful_stop` 2 s plus `forced_reap` 1 s, `shutdown_deadline` 5 s; `packages/brawler-routing/src/supervisor/process.rs:92`). It ends running matches; no drain mode exists.
- Matches last up to `match_duration_seconds: 180` plus countdown, a loading deadline of up to 30 s and results (`config/server/game-types.ron`). A drain therefore needs up to about 4 minutes. Rejoin seats (`src/server/lobby/rejoin.rs`) end with their match.
- The lobby already stops forming matches when `free_match_slots == 0` (`src/server/lobby/mod.rs:781`, `:2024`). The drain can reuse that gate and needs no new formation logic.

# Decisions (owner-approved 2026-11-21)
1. **One compatibility identity.** Drop the exact `build_version` equality from the gate (keep it for logs and diagnostics). Compatibility is the protocol version plus the registry and content fingerprints, matching docs/08's single global handshake.
2. **Update-required state.** Every compatibility rejection maps to one player-facing state: "A new version of PewPew Blitz is required to play on this server." On Android it offers "Update" (opens the Play Store listing through a small JNI call that mirrors the existing `startActivity` use). Desktop shows the text only. With BRL-z277, the token service returns the same rejection before any UDP connect.
3. **Drain mode.**
   - A supervisor drain request (SIGUSR1, or a second signal-free mechanism chosen at implementation), plus a drain deadline flag defaulting to 300 s.
   - On drain, the supervisor advertises zero free match slots to the lobby, and the lobby shows queued and Practice players the existing capacity message.
   - Running matches finish normally; then the ordinary graceful shutdown runs.
   - Matches still running at the deadline are stopped.
   - The container stop timeout (BRL-sbep) becomes deadline plus 10 s for a deploy; a plain `docker stop` keeps the fast path.
4. **Deploy procedure** (in `docs/19-server-operations.md`): pull the new image, drain, stop, start the new image on the same volume, smoke-check. Release ordering: a server change that alters compatibility ships only after the matching Play Store build is approved and rolled out. The server update then moves every player to the new version at once.
5. **versionCode:** derived at build time (for example from a monotonically increasing release number passed to `just android-bundle`), never hand-edited.

# Scope
Items 1–5. One protocol version bump if the rejection payload changes.

# Non-goals
Running two server versions side by side, rolling or blue-green deploys, in-app forced updates (Play In-App Updates API), and a desktop auto-updater.

# Acceptance
- A real-process test: a client built with a different content fingerprint reaches the update-required state.
- A real-process drain test: with one match running, a drain request lets it reach Result. New Practice starts are refused with the capacity message during the drain, and the supervisor exits 0 afterward.
- `just android-bundle` produces increasing `versionCode`s without manual edits.

# Verification
`just lint`, `just test`, the new real-process tests, and a manual Android check that "Update" opens the store listing (needs a published or internal-track listing; record if unavailable).
