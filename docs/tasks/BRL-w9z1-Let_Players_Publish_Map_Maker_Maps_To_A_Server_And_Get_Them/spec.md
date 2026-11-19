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

# Recommended design (2026-11-19)

Design research only; nothing is implemented. Grounded in current source and docs/04, 08, 13, 14, 15, 16.

## What already exists (research findings)

Private authored Practice (BRL-6vjz, protocol 47) already built most of the transport path a published map needs:

- `src/map/authoring.rs:111` `validate_authored_recipe` is the authoritative validator for player recipes. It accepts Wipeout, Hot Zone and Heist only (not Zombie). It preflights dimensions, source items, expanded placements and canonical bytes against `AUTHORING_LIMITS` (20..=128 cells per axis, 16,384 cells, 4,096 expanded placements, 256 KiB source) before it calls the shared resolver.
- `src/server/lobby/mod.rs:256-300` `resolve_practice_map` runs that validator and the exact roster/spawn-capacity check in the lobby, then binds the postcard recipe bytes, revision and fingerprint to the allocation (`AllocateRequestBody.map_recipe`).
- Match-manifest v5 carries the immutable recipe through the supervisor without interpreting it. `src/server/admission.rs:700-713` decodes it into `ServerMapSelection { preset_id: None, authored_recipe }`, and `src/map/server.rs:88-93` revalidates and installs it through ordinary map runtime.
- Clients rebuild the map from the replicated `ResolvedMapSnapshot` (`replicate_once`, `src/protocol.rs:628`, `src/map/client.rs`). They never need the recipe in their local catalog. `ReservationStarted` (`src/lobby/queue.rs:67-80`) already allows `map_preset_id: None` with recipe id, revision and fingerprint.
- Queue rotation is round-robin per game type: `src/server/lobby/queue.rs:996-1014` picks `game.map_preset_ids[ordinal % len]`. Queue allocation (`src/server/lobby/mod.rs:755-772`) still resolves only presets and sends `map_recipe: Vec::new()`.
- Pools come from `maps: [...]` in `config/server/game-types.ron` (at most `MAX_MAPS_PER_GAME_TYPE = 8`, `src/lobby.rs:26`). They are advertised in `AdvertisedGameType.map_preset_ids` (`src/lobby.rs:149`), and the canonical bytes form `CatalogRevision` (`src/server/lobby/catalog.rs:414`). Queue tickets must match that revision (`queue.rs:627`).
- The recipe fingerprint (`src/map/resolution.rs:370-390`) hashes `recipe_id` and `revision` as well as the layout. Client draft IDs become `recipe_id` (`packages/brawler-client/src/map_maker/model.rs:43-50`). Two authors' recipes therefore have unrelated IDs, but nothing guarantees they are unique.
- The map name lives only in the client draft envelope. `normalize_map_name` is client code, and `MAX_AUTHORED_MAP_NAME_*` already lives in `brawler::map::authoring`. `MapRecipe` has no name field (docs/16).
- Profiles live in a lobby-owned SQLite database (WAL, `busy_timeout` 2 s, `user_version` 2, `src/profiles/storage.rs:18,64-65`), behind the single bounded storage executor, at the lobby manifest's `profile_database_path` inside the server data directory.
- `brawler-profile-admin` (`src/bin/profile_admin.rs`) is an offline CLI that only supports `backup --database --output`.
- Balance Lab (docs/15) is a development-only loopback HTTP console that only Practice match workers host. It has no lobby-lifetime presence and no authentication.
- Accounts are an unauthenticated development seam (docs/13:425-433). The client generates an opaque `AccountId` for each logical server, and there is no recovery or creation rate limit. Display names are untrusted presentation that the lobby normalizes and de-duplicates per session.

## Answers to the seven questions

### 1. Submission — an in-client "Publish to this server" action over the routed lobby connection

Recommendation: from My Maps or the editor, Publish sends one new lobby request, `MapPublishRequest { request_id, catalog_revision, game_type_id, game_type_configuration_revision, name, candidate: AuthoredMapCandidate }`, on the existing lobby session. The lobby replies `MapPublishOutcome { request_id, Accepted { published_map_id, state } | Rejected(reason) }`. A small `MyPublishedMaps` request and response lists the caller's submissions with their states, and `WithdrawPublishedMap { published_map_id }` withdraws one.

Rationale: `PracticeStartRequest` already carries a bounded authored recipe of up to 256 KiB over this same connection, so the size and decode bounds are proven. The submitter is the lobby session's `AccountId`. The client cannot claim an author, an ID or a state. There is no separate upload service, HTTP endpoint or asset upload (docs/04 and docs/08 forbid asset upload). The author picks a target game type, exactly like Test in Practice, because spawn capacity is roster-specific.

### 2. Trust and moderation — operator pre-approval by CLI (owner question Q1)

Recommendation: every accepted submission is stored as `Pending`. An operator decides with new `brawler-profile-admin maps` subcommands against the server data directory's database:

- `list [--state pending]` shows ID, name, author display name, account, game type, dimensions, counts, submitted time and fingerprint.
- `approve <id>`.
- `reject <id> --reason <text>`.
- `remove <id>` takes an approved map out of rotation.
- `export <id> --output <file>` writes a Map Maker draft envelope, so an operator can open the map and test it in Practice.

Offensive map names are handled by moderation: the operator rejects the map, and its name is never shown to other players while it is pending. Names reuse the shared normalizer, which this work moves from the client into `brawler::map::authoring` so the lobby enforces the same 48-grapheme / 96-byte rule.

Rationale: accounts are unauthenticated, so any automatic trust can be farmed with fresh account IDs. Pre-approval is the only gate that does not depend on identity. The CLI already owns the database operationally (backup). Balance Lab is the wrong surface: it is a Practice-worker loopback page with no lobby lifetime, and making it a moderation console would mean adding authentication. Community voting needs real identity and is deferred.

### 3. Validation — the same authoritative validator, run three times

Recommendation:

1. **At submission**, the lobby runs exactly the `resolve_practice_map` sequence: preflight, `validate_authored_recipe` for the game type's mode, and spawn capacity for that game type's exact roster. It rejects with the existing `InvalidMapRecipe` and `RosterMismatch` reasons plus new publishing reasons (`InvalidName`, `QuotaReached`, `CoolingDown`, `PublishingDisabled`, `StoreFull`, `StaleCatalog`).
2. **At lobby startup and on every rotation refresh**, the lobby revalidates each approved map against the current embedded `MapContentCatalog` and the current game type. A map that no longer validates becomes `Suspended(ContentChanged | RosterChanged)` and leaves rotation. It is never deleted or migrated, which follows the BRL-6vjz rule that stale content fails closed and stays recoverable.
3. **In the match worker**, the existing manifest-v5 revalidation runs unchanged.

Clients only ever send an `AuthoredMapCandidate`. The server never accepts geometry it cannot check. One lobby-owned helper is extracted from `resolve_practice_map` so that Practice, publishing and queue allocation share one code path instead of three copies.

### 4. Fingerprint and compatibility — published maps stay outside both fingerprints

Recommendation:

- Published recipes are server data, like profiles. They never contribute to `GameplayContentFingerprint`, because the shared map-content catalog, which already covers every asset and profile ID a recipe can reference, is unchanged.
- They are also left out of `AdvertisedGameType.map_preset_ids` and `CatalogRevision`. If they were included, every approval would change the catalog revision and turn players' in-flight queue joins into stale-catalog failures.
- The recipe reaches the worker in the manifest-v5 `map_recipe` field that already exists. Clients receive it as the ordinary replicated `ResolvedMapSnapshot`, so there is no client-side map distribution, cache or download.
- Identity: when the lobby accepts a submission it assigns a server-unique `PublishedMapId` and rewrites `recipe.recipe_id` and `revision` from it before it fingerprints and stores the recipe. Recipe fingerprints are then unique per published revision on that server. The stored row keeps the canonical postcard bytes, the recipe fingerprint, and the gameplay-content fingerprint it was accepted under.
- Protocol: the new lobby request and response types, plus a credit field on `ReservationStarted`, need one `SUPPORTED_PROTOCOL_VERSION` bump (51) and change the protocol-registry fingerprint, through the global handshake with no per-message decoder. Match-manifest v5, control IPC v7, the route envelope and the supervisor are unchanged.

### 5. Rotation — approved maps join their game type's round-robin, per-game-type operator opt-in (owner question Q2)

Recommendation: a new operator field in `config/server/game-types.ron`, `community_maps: true` per game type, with `false` as the default. For game types with the flag, the lobby keeps an in-memory `CommunityRotation` (the approved maps per game type, in stable approval order, capped at 64 per game type in code). The queue's existing `map_ordinals` round-robin then runs over built-in presets followed by community maps.

`QueueReservation.map_preset_id` becomes a two-variant map source (preset or published map), and queue allocation sends the stored recipe bytes exactly as Practice does. The lobby refreshes the rotation from storage on a bounded interval (about 5 s, converted through `src/timing.rs`) and after its own writes, so CLI decisions take effect without a restart. An approval affects only reservations formed afterwards. A reservation already formed keeps its immutable manifest.

Rationale: this reuses the one rotation mechanism that exists, does not split queue population across new game types on small servers, and keeps built-in content in place. It is a real product choice, so it is asked as Q2.

### 6. Ownership and attribution — the account owns it, and a display-name snapshot is the credit

Recommendation:

- Each row stores the submitting `AccountId`, the author's accepted display name at submission time, the map name, the game type ID and revision, the state, timestamps and the rejection reason.
- `ReservationStarted` gains `published_map: Option<PublishedMapCredit { published_map_id, name, author_display_name }>`. Match Loading and Results then show "Map name — by Author" for a community map.
- The author withdraws from My Maps (`Withdrawn` leaves rotation immediately; matches already running are unaffected). Resubmitting the same draft while a submission is pending replaces that pending row.
- Rows live in the existing profile SQLite file in the server data directory, so the lobby's single storage executor remains the only in-process writer and backups cover maps automatically. This needs schema migration 2→3 adding a `published_maps` table, plus executor commands.

Limitation recorded honestly: "owner" means "holder of that device's AccountId" until real authentication exists. The UI must not imply a protected creator identity.

### 7. Abuse limits

Recommendation:

- **Code ceilings** (representation and capacity): the existing 256 KiB source and authoring limits; at most 256 stored non-terminal rows per server; at most 64 pending rows per server (`StoreFull` beyond that); at most 64 rotation maps per game type; bounded name and reason strings.
- **Operator policy** in `game-types.ron` `map_publishing: (enabled, pending_per_account, published_per_account, submit_cooldown_seconds)`. Proposed defaults: enabled, 3 pending, 10 approved, 60 s cooldown (owner question Q3).
- **Rate limiting**: one in-flight publish request per lobby session, plus the cooldown per account.
- **Removal**: an operator uses `remove`. Rejected and withdrawn rows are kept for audit until an operator purges them, a CLI command that can follow later.

Because accounts are free, the per-server pending cap, not the per-account quota, is the real bound.

## Invariants kept

- The server stays authoritative: the lobby validates, the worker revalidates, and clients send only a typed candidate.
- No published map changes `GameplayContentFingerprint` or `CatalogRevision`, so compatible clients are never split.
- One global protocol bump with no per-message versions; manifest and IPC are unchanged.
- No executable content, asset upload or network references.
- Publishing and rotation live in lobby-owned modules: authority beside `src/server/lobby/`, and storage in the profile executor. No mode, collision or map-runtime system branches on whether a map is a community map (docs/04).
- Operator policy goes in validated `config/server/game-types.ron`, and ceilings go in code.

## Smallest first slice (one playable published map)

1. **Shared**: move map-name normalization into `brawler::map::authoring`. Extract the lobby's authored-map acceptance helper from `resolve_practice_map` so Practice, publishing and queue allocation share it.
2. **Storage**: migrate the profile database to schema 3 with a `published_maps` table, add executor commands, and a server-assigned `PublishedMapId`.
3. **Lobby**: handle `MapPublishRequest` (validate, check quota and cooldown, store as `Pending`), keep a `CommunityRotation` refreshed on an interval, and add the queue map-source variant plus recipe-bytes allocation for game types with `community_maps: true`.
4. **CLI**: `brawler-profile-admin maps list | approve | reject` (with `remove` if it is trivial).
5. **Client**: a Publish action in My Maps that offers the draft's compatible advertised game types, shows the outcome and reason, and shows the map credit on Match Loading.
6. **Config**: enable `community_maps` on one checked-in game type, for example `wipeout-1v1`, for playtesting.
7. **Docs and tests**:
   - docs/04, 08 (protocol 51), 13, 14 and 16 describe the flow.
   - Unit tests cover storage migration and quota.
   - Separate-App network tests cover submit-accept, validation rejection, and a queued match on an approved published map, where every client reaches map readiness with the published fingerprint.
   - A routed e2e covers publishing on client A, approving with the CLI, then a queued 1v1 that clients A and B both load.

Deferred to backlog tickets once the design is approved: the "Your submissions" status screen with withdraw (slice 4 in the original list), Results credit, choosing published maps in other players' Practice picker, an in-client operator preview, voting or ratings, replacing an approved revision with an update, purge, and real account authentication.


# First slice: implementation decisions and deviations (2026-11-19)

Owner answers (questions list) override the recommended design: per-server `map_publishing.moderation: Approve | Automatic` (default Approve), per-game-type `community_maps: true`, 3 pending per account, 120 s cooldown, no approved cap. Research claims in the recommended design were checked against the code and hold (validator, manifest v5 path, worker revalidation at `src/server/admission.rs` `validate_manifest_map_against_catalog`, queue round-robin, profile SQLite executor).

Deviations from the recommended design, recorded before they shipped:

1. **Operator config.** `map_publishing: (moderation, pending_per_account, submit_cooldown_seconds)` and per-game-type `community_maps` are optional fields with defaults (Approve, 3, 120 s, false). The operator catalog schema stays 6 because older catalogs still parse with the same meaning. `enabled` and `published_per_account` are not added: opt-in is per game type and the owner removed the approved cap. `wipeout-1v1` opts in and its configuration revision moves 2 -> 3.
2. **`AdvertisedGameType.community_maps`** is advertised (in the canonical catalog bytes) so Map Maker can pick an opted-in target. It changes the catalog revision only when operator config changes, never on approval.
3. **Storage.** One `published_maps` table in the profile database (schema 3, forward migration from 0/1/2). The storage executor's reply becomes `ProfileStorageReply { Profile | PublishedMapSubmission | PublishedRotation }`; `ProfileAuthority` routes map completions to the lobby's publishing systems. Quota, cooldown (from the last accepted submission), and capacity are checked inside the insert transaction. Code ceilings: 256 stored pending+approved, 64 pending, 64 rotation per game type, 256-byte reason.
4. **No persisted `Suspended` state.** An approved row that stops validating, or whose game type changes revision or drops the opt-in, is left out of the in-memory rotation and logged; the row stays `approved` and recoverable. Simpler and equally fail-closed.
5. **No withdraw, no "my submissions" request, no resubmit-replaces-pending.** Each accepted publish is a new row. These belong with the deferred submissions screen.
6. **Rejection reasons:** `InvalidName, InvalidMapRecipe, RosterMismatch, StaleCatalog, UnknownGameType, StaleGameConfiguration, CommunityMapsDisabled, PendingLimitReached, CoolingDown { retry_after_seconds }, StoreFull, Busy, Internal`.
7. **CLI:** `maps list [--state]`, `approve`, `reject --reason`, and `remove [--reason]` ship; `export` is deferred.
8. **Client:** one PUBLISH button per My Maps card (current style), status line shows the outcome; Match Loading adds one "Map — by Author" line. No restyle (BRL-2xpc owns the redesign).
9. **Protocol 51** (global handshake): `MapPublishRequest`, `MapPublishOutcome`, `AdvertisedGameType.community_maps`, `ReservationStarted.published_map`. Manifest v5, control IPC v7, route envelope unchanged.


# First slice: implementation, verification, and learning (2026-11-19)

Implemented on branch `worktree-agent-ad51b6905ae27cf81`, commit 49616ef6 (on main ad565170). Protocol 51.

## Built

- Shared: `brawler::map::authoring::normalize_map_name` (moved from the client); wire types in `src/lobby/publishing.rs`; `AdvertisedGameType.community_maps`; `ReservationStarted.published_map`.
- Storage: `src/profiles/storage/published_maps.rs` (schema 3 table, quota/cooldown/capacity in one transaction, operator decisions, rotation order = decision time then ID). Executor reply enum; `ProfileAuthority` routes map completions.
- Lobby: `src/server/lobby/map_publishing.rs` (shared `accept_authored_map` now also used by Practice, publish request handling, 5 s rotation reload with re-validation, `CommunityMapRotation`); `QueueMapSource { Preset | Published }` round-robins built-in maps then community maps; product allocation sends recipe bytes with `map_preset = 0`; Match Loading credit.
- Operator: `map_publishing` section and per-game-type `community_maps` in `config/server/game-types.ron` (wipeout-1v1 opted in, revision 3); `brawler-profile-admin maps list | approve | reject | remove`.
- Client: PUBLISH on each My Maps card (current style), outcome text on the library status line, "Map — by Author" on Match Loading. Headless automation `--product-publish-map-smoke` and `--expect-published-map`.
- Docs 04, 08, 13, 14, 16 and README updated. New `just published-map-e2e` (`scripts/e2e-published-map.sh`).

## Finding beyond the spec

The research section claimed clients already rebuild an authored map from the replicated snapshot. They did not: `src/map/client.rs` rejected every snapshot without `source_preset_id` ("replicated map has no preset identity"), so an authored Practice or published map never became `ClientMapReadiness::Ready` on a real client. The e2e exposed it. The client now rebuilds the canonical recipe from the snapshot, re-resolves it with the same resolver, and requires the identical snapshot (fingerprint included). The network test now also requires `ClientMapReadiness::Ready`.

## Verification

- `just lint` (ripgrep on PATH): pass.
- `just test`: routing, client (910), server (561 + 611 lib), balance-lab, and network (134, including the 2 new `map_publishing` tests) suites pass. `_test-performance` fails on wall-clock budgets only while run in parallel on a machine at load average ~26 from other agents' builds (p95 up to 38 ms); run serially (`cargo test -p brawler-client --features network-test --test performance -- --test-threads=1`) all 12 pass with p95 <= 10.1 ms against 16.7 ms. No changed code runs in those per-tick systems. Rerun `just test` on a quiet machine.
- New tests: storage quota/cooldown/decisions/rotation order, Automatic moderation, v2 -> v3 migration (`src/profiles/tests.rs`); catalog policy defaults/opt-in/invalid values; queue rotation built-in -> community -> built-in with stale/closed rows dropped; wire bounds (`src/lobby/publishing.rs`); protocol 51; client snapshot re-resolution; client publish model and credit text; network: validation rejections (wrong mode, oversized, closed game type), pending storage with server-assigned identity, cooldown, and an approved map reaching the second 1v1 reservation, the allocation request, Match Loading credit for both clients, and a production match worker that both clients load to `Ready`.
- E2E: `BRAWLER_ROUTED_BIND=127.0.0.1:5082 BRAWLER_SERVER_DATA_DIR=$PWD/target/w9z1/server ./scripts/e2e-published-map.sh` passed: client A published map 1 (pending), `maps list` showed it, `maps approve 1`, the first 1v1 played the built-in map (recipe_id 7), the second showed `credit="E2E Published Yard — by Brawler-1BBD" published_map_id=1` on both clients and both reached Active on recipe_id 281474976710657 (2^48 + 1).

## Not verified

- No windowed check of the PUBLISH button or the Match Loading credit line (no windowed clients were launched). Playtest: `just server`, `just client`, open Map Maker, save a valid Wipeout draft, press PUBLISH on its card (expect "Published. The server operator reviews it…"), run `cargo run --no-default-features --features server --bin brawler-profile-admin -- maps approve 1 --database target/dev/server/profiles.sqlite3`, then play two 1v1 queue matches with two clients: the second shows "<map> — by <name>" on Match Loading.

## Deferred (linked backlog)

BRL-s3dh (submissions status + withdraw), BRL-1sgj (Results credit), BRL-r0ky (Practice picker), BRL-76a9 (voting/rating), BRL-4gqt (update an approved map).

## Mistakes and prevention

- Trusted the spec's claim that clients rebuild authored snapshots; only the real-process e2e caught it, because the network test checked replicated identity rather than client readiness. Prevention: map-distribution tests assert `ClientMapReadiness::Ready`, not just the replicated component.
- The e2e cleanup referenced an empty bash array under `set -u` (macOS bash 3.2) and leaked a supervisor on failure; fixed with `${arr[@]+...}` and killed the leaked process by PID.
- Adding a flow system changed entity order and exposed a tie in `flow_button_sizes` (two Dashboard buttons share `OpenBrawlerList`); the sort now has a total order.
