# V7 Milestone 01 — Durable profile and saved-brawler loop

## Status

`User playtest`

Research and specification planning were authorized on 2026-08-22 after V6 completed and was
accepted. The user approved starting implementation on 2026-08-22, accepting the five review
recommendations below. Implementation must reconcile the V7 content cutover with the accepted V6
Balance Lab rather than assuming that its current build-preset and point-budget surfaces remain
valid.

## Player-visible outcome

A new client identity can enter the existing routed lobby, atomically load or create its
logical-server-local profile, create and manage saved brawlers with permanent fighter-profile and
weapon-base choices, select one, recover it after client and server restart, and enter Practice or
multiplayer using the exact immutable admitted loadout.

Weapon-part inventory and four-slot equipment remain M02. This milestone establishes the durable
profile, saved-brawler, Dashboard, queue-freeze, and match-handoff contract they will extend.

## Accepted product decisions

See the [V7 roadmap](./roadmap.md#product-decisions-already-accepted). M01 preserves those decisions
without reopening production authentication, acquisition, cloud profiles, or weapon-part scope.

## Research findings

### Current Brawler implementation

- [`src/builds/model.rs`](../../../src/builds/model.rs) models one transient build selection: a
  weapon choice, ultimate, and two passives. It has no account, saved-brawler identity, fighter
  profile selection, inventory, or durable server owner.
- [`src/builds/definitions.rs`](../../../src/builds/definitions.rs) owns the 12-point budget, four
  named presets, and an implicit fighter-stat rule: selecting Lightweight Frame or Reinforced Frame
  changes the resolved stat profile. The two frame passives therefore duplicate the creation-time
  fighter-profile concept V7 needs.
- [`content/v1/builds.ron`](../../../content/v1/builds.ron) already defines three viable stat
  profiles: Default, Lightweight, and Reinforced. Combat still uses one standard fighter/body
  definition, so these are gameplay profiles of one body rather than different physical bodies.
- [`src/client/build_persistence.rs`](../../../src/client/build_persistence.rs) stores one local
  `build.ron`. V7 must cease reading and writing it but leave an existing file untouched, consistent
  with the accepted fresh-start policy.
- The current Dashboard and build editor assume four named presets, Custom Pulse, and a point
  budget. M01 must replace that product flow; it cannot wrap persistence around the existing editor.
- [`src/protocol.rs`](../../../src/protocol.rs) has one global compatibility handshake and currently
  sends a `LobbyHello` without persistent identity. Queue commands carry a client-authored build.
  V7 instead needs an account-bearing hello and server-owned saved-brawler resolution, with one
  normal protocol/schema bump and no compatibility decoder.
- [`src/server/lobby/mod.rs`](../../../src/server/lobby/mod.rs) accepts a lobby hello synchronously,
  installs a transient default build, validates later client-authored build candidates, and freezes
  an opaque build snapshot into the match allocation. The opaque snapshot seam remains useful, but
  profile loading cannot block this schedule.
- [`packages/brawler-routing/src/manifest.rs`](../../../packages/brawler-routing/src/manifest.rs)
  already transports bounded application-owned match-build bytes opaquely. Its participant metadata
  still exposes preset identity and must be revised when presets disappear.
- [`packages/brawler-routing/src/bin/supervisor.rs`](../../../packages/brawler-routing/src/bin/supervisor.rs)
  generates a new `LogicalServerId` on every process start. That is incompatible with a client
  storing one account identity per logical server and must become durable configuration.
- [`src/client/connection_persistence.rs`](../../../src/client/connection_persistence.rs) provides a
  bounded, versioned, atomic local persistence pattern for saved servers. Its schema can be advanced
  to store one generated account ID for each known logical server without introducing another
  client-side file owner.
- The V6 Balance Lab uses one dedicated standard thread and bounded channels for non-ECS work. That
  is a useful local lifecycle pattern, but profile storage remains a separate owner and must never
  share the development-only HTTP or tuning state.

### SQLite and Rust dependency research

- SQLite write-ahead logging allows readers to continue while a writer commits, but the `-wal` file
  is part of the database's persistent state. A raw copy of only the main database is therefore not
  an acceptable backup. See the official [WAL documentation](https://www.sqlite.org/wal.html) and
  [online backup documentation](https://www.sqlite.org/backup.html).
- SQLite exposes `application_id` for file identification and `user_version` for an application-owned
  schema revision. `integrity_check` does not report foreign-key violations, so recovery checks must
  pair a database integrity check with `foreign_key_check`; foreign-key enforcement must also be
  enabled explicitly. See the official [PRAGMA reference](https://www.sqlite.org/pragma.html).
- `rusqlite` 0.40.2 is the researched implementation candidate. Its `bundled` feature gives the
  application a known SQLite build and its optional `backup` feature exposes SQLite's online backup
  API. Its transactions roll back on drop unless committed. See the current
  [`rusqlite` crate documentation](https://docs.rs/rusqlite/latest/rusqlite/) and
  [`Backup` API](https://docs.rs/rusqlite/latest/rusqlite/backup/struct.Backup.html).
- SQLx provides SQLite pooling and migrations, but its pool requires an async runtime and creates
  connection-pool machinery that this single-owner design does not need. A synchronous `rusqlite`
  connection on one dedicated storage thread is the smaller boundary. See the current
  [SQLx pool documentation](https://docs.rs/sqlx/latest/sqlx/pool/).
- No generic repository, ORM, async runtime, or database pool is justified for M01. Embedded,
  forward-only SQL migrations and a focused typed storage command API are sufficient.

## Decisions from research

### Launch fighter profiles

M01 should promote Default, Lightweight, and Reinforced into the three permanent fighter-profile
choices at brawler creation. They reuse the already-authored, tuned stat profiles and provide a real
choice without inventing new bodies or content.

Lightweight Frame and Reinforced Frame should simultaneously leave the swappable passive catalog.
Keeping them would create two competing ways to select the same fundamental stats and could allow
contradictory combinations. Adrenal Response, Close Quarters, Quick Cycle, and Tenacity remain the
initial freely swappable passives. This is a specification recommendation requiring user acceptance.

### Empty-profile flow

An account begins with no saved brawlers. The Dashboard opens creation immediately and preselects
safe defaults for fast controller use, but the player must confirm creation; there is no hidden
starter template. A generated, editable display name such as `Brawler 1` avoids requiring keyboard
input. The first created brawler becomes selected automatically.

Deleting the selected brawler selects the remaining brawler with the lowest stable creation ordinal,
or leaves the profile empty if none remain. This is deterministic and avoids a persistent dangling
selection. These behaviors require user acceptance.

### Simultaneous use of one insecure account ID

M01 permits at most one active lobby session for an `AccountId`. A second session presenting the
same ID is rejected while the first remains active. This avoids last-writer ambiguity and accidental
profile sharing without pretending the caller-supplied ID is secure. Reconnect after the first
session is gone loads the same profile normally. This policy requires user acceptance.

## Technical specification

### Identities and trust boundary

- `AccountId` is a nonzero opaque 128-bit value on the wire and is displayed/persisted as 32
  lowercase hexadecimal digits. A client generates it from OS entropy; deterministic tests may
  construct it directly. Format validation is not authentication.
- `SavedBrawlerId` is a server-generated nonzero 128-bit identity. It remains distinct from
  `AccountId`, match `PlayerId`, `NetworkEntityId`, routed IDs, and Bevy `Entity`.
- One profile is keyed directly by `AccountId`; M01 does not add an unused `ProfileId` layer.
- The supervisor receives a stable logical-server data directory. On first startup it creates a
  durable logical-server identity file atomically; later startups load it. It must reject malformed
  or unreadable identity data rather than generating a replacement that strands profiles.
- A client initially knows only an address. After the transport connects, the lobby first announces
  its public stable `LogicalServerId`. The client looks up the account ID bound to that logical
  server, or generates one, and then sends `LobbyHello`. This small pre-authentication exchange lets
  the same server move addresses without creating another client identity. Address aliases may be
  updated only after the server ID is known; a conflicting saved alias must not overwrite another
  server's identity silently.
- The client connection file advances as one bounded atomic unit. Corrupt local identity storage is
  reported and preserved, not silently reset. Losing it has the already-accepted consequence of
  creating another account on the next connection.

### Saved-brawler model

Persist authored choices, not resolved runtime values:

```text
SavedBrawler
  id: SavedBrawlerId
  creation_ordinal: u64
  name: bounded normalized presentation string
  fighter_profile_id: FighterProfileId       # immutable
  weapon_base_id: WeaponBaseId               # immutable
  ultimate_id: UltimateDefinitionId          # mutable outside queue
  passive_ids: [PassiveDefinitionId; 2]       # mutable outside queue
  revision: nonzero integer
```

- Names reuse the existing player-name normalization and bounds, may be duplicated, and are not a
  lookup key.
- `FighterProfileId` becomes an explicit authored selection. It is not inferred from passives.
- `WeaponBaseId` replaces `WeaponChoice`/Custom Pulse as the saved creation fact. The four current
  complete weapon presets become the four bases. M01 offers no custom numeric weapon editor.
- The two passive slots remain distinct selections but may not contain the same passive unless the
  authored passive definition explicitly supports duplication; the initial catalog does not.
- Ultimate and passives are validated against the active catalog on every mutation and again at
  queue admission. A content update that makes a saved selection unsafe rejects queue admission and
  reports the exact repair needed; it never rewrites owned data silently.
- The profile owns a nonzero monotonic revision. Every accepted mutation advances it. Each brawler
  also has a nonzero monotonic revision so queue admission and UI updates can identify the exact
  accepted brawler state.

### Database schema and migrations

M01 creates only the tables it owns now; M02 adds inventory and equipment through a later migration.

```text
profiles(
  account_id BLOB(16) PRIMARY KEY,
  revision INTEGER NOT NULL,
  next_brawler_ordinal INTEGER NOT NULL
)

brawlers(
  account_id BLOB(16) NOT NULL,
  brawler_id BLOB(16) NOT NULL,
  creation_ordinal INTEGER NOT NULL,
  name TEXT NOT NULL,
  fighter_profile_id INTEGER NOT NULL,
  weapon_base_id INTEGER NOT NULL,
  ultimate_id INTEGER NOT NULL,
  passive_1_id INTEGER NOT NULL,
  passive_2_id INTEGER NOT NULL,
  revision INTEGER NOT NULL,
  PRIMARY KEY(account_id, brawler_id),
  UNIQUE(account_id, creation_ordinal),
  FOREIGN KEY(account_id) REFERENCES profiles(account_id) ON DELETE CASCADE
)

profile_selection(
  account_id BLOB(16) PRIMARY KEY,
  brawler_id BLOB(16) NOT NULL,
  FOREIGN KEY(account_id, brawler_id)
    REFERENCES brawlers(account_id, brawler_id) ON DELETE CASCADE
)
```

- SQL constraints cover byte lengths, positive revisions/ordinals, and bounded scalar fields where
  SQLite can express the invariant. Rust validation remains authoritative for catalog references,
  normalized text, counts, and semantic combinations.
- The database has one fixed nonzero `application_id`. `user_version` is the schema version.
- Forward-only migration SQL is embedded in the server binary. Each migration and its `user_version`
  update commit in one transaction. A database newer than the binary is rejected without writes.
- Opening an existing database verifies the application ID, schema version, integrity, foreign keys,
  and semantic decoding before the lobby becomes ready. An empty new file receives the application
  ID and initial migration transactionally.
- Structural corruption, an unexpected application ID, a newer schema, migration failure, or an
  unreadable database prevents lobby readiness and preserves the database plus WAL/SHM files.
  A malformed individual record rejects that account's load or mutation and reports the fault; it
  is not deleted, defaulted, or partially returned.

### Storage process ownership

`profiles/` becomes one focused server-owned module:

```text
src/profiles/
  mod.rs          shared IDs, bounded snapshots, commands, outcomes, plugin surface
  model.rs        profile and saved-brawler authored state plus validation
  protocol.rs     reliable profile request/snapshot shapes if not kept in root protocol.rs
  authority.rs    lobby-only accepted cache, revisions, session/queue coordination
  storage.rs      server-only rusqlite owner, migrations, backup primitives, tests
  client.rs       client cache and Dashboard actions
  tests.rs        focused model/authority composition tests
```

- A named dedicated thread exclusively owns one `rusqlite::Connection` and all statements.
  Bevy systems communicate through bounded `std::sync::mpsc::sync_channel` command/result queues;
  no Bevy schedule performs blocking database work.
- The lobby creates the storage owner during startup and does not advertise readiness until open,
  migration, and validation succeed. Startup failure exits the lobby worker so the supervisor's
  existing lifecycle machinery can report/restart it; it must not fall back to an in-memory profile.
- ECS submission is nonblocking. A full command queue rejects the player operation as temporarily
  unavailable without changing accepted state. The bound starts at 64 commands/results and is a
  capacity guard, not a profile-creation rate limit.
- Runtime executor disconnect/panic is fatal to profile authority: stop accepting connections,
  profile mutations, and queue admission, then fail the lobby worker. Existing matches remain
  isolated and continue from their admitted snapshots.
- Shutdown stops new submissions, drains/cancels outstanding operations deterministically, sends a
  shutdown command, closes the connection, and joins the thread within the worker lifecycle.
- The supervisor passes a validated profile database path only in the lobby manifest/configuration.
  It never opens SQLite. The shared headless worker binary may link the server dependency, but match
  composition receives neither a database path, `ProfileAuthority`, nor a storage connection and
  performs no profile I/O.

### Application transactions and concurrency

`ProfileAuthority` is the only application-level owner of accepted mutable profile state.

- The lobby sends its stable logical-server identity after transport connection. The account-bearing
  client hello then creates a pending session and submits `LoadOrCreate(AccountId)`.
  `LobbyJoinOutcome::Accepted` is emitted only after storage returns a fully validated profile.
  Storage rejection produces a bounded join rejection and no authenticated lobby session.
- Load-or-create is one SQLite transaction: insert the absent profile with `ON CONFLICT DO NOTHING`,
  then read the canonical row and its saved brawlers. Caller-supplied valid unknown IDs are expected
  and create profiles idempotently.
- At most one storage mutation per active account is in flight. Commands carry a nonzero request ID,
  expected profile revision, and where relevant expected brawler revision. Duplicate/stale requests
  return the already-known result or a stale decision; they never apply twice.
- Create, edit, select, and delete validate against the in-memory accepted snapshot, execute one
  database transaction, and update the accepted cache only after commit succeeds. Failure leaves
  both the database transaction and accepted cache unchanged.
- The 16-brawler cap is checked inside the same create transaction, not only in the client or cache.
- Queue admission is serialized with mutations. It is accepted only when no profile command is in
  flight; authority marks the session queued before resolving and copying the selected brawler.
  Every subsequent edit/delete/select request from that session is rejected until it leaves queue.
- The accepted queue snapshot is immutable. A running match never observes later profile changes
  and never needs an account or database connection.

### Network protocol

- Advance the one global protocol/schema version. Do not add per-message versions or legacy
  compatibility decoding.
- Add a bounded public `LobbyServerIdentity` message sent before `LobbyHello`; extend `LobbyHello`
  with `AccountId`. Repeat `LogicalServerId` in the accepted welcome and include the complete bounded
  profile snapshot so the client can bind the response to the pre-authentication announcement and
  the Dashboard has one authoritative initial state.
- Add one reliable ordered `ProfileChannel` with bounded `ProfileCommand` and `ProfileOutcome`
  messages. Commands are `CreateBrawler`, `EditBrawler`, `SelectBrawler`, and `DeleteBrawler`.
  Outcomes contain the request ID, a typed decision, and on success the complete new bounded M01
  profile snapshot. With at most 16 small brawlers, whole-snapshot replacement is simpler and safer
  than client-side patch reconciliation.
- Bound strings, vectors, and total encoded snapshot size before accepting or allocating. Reject
  malformed IDs, unknown content IDs, immutable-field edits, stale revisions, invalid combinations,
  queue-locked commands, and oversized candidates as typed decisions.
- Replace queue requests that carry arbitrary `BuildCandidate` data with a selected
  `SavedBrawlerId` plus expected brawler revision. The lobby resolves only its accepted profile
  state against the active catalogs.
- Replace `MatchBuildSnapshotV1` with one bounded V2 snapshot containing the canonical authored
  brawler recipe, resolved/fingerprint identity, and content revision needed by the match worker.
  Routing continues to treat these bytes opaquely. Remove preset-specific participant metadata and
  do not add `AccountId` to the match manifest.

### Content resolution and Balance Lab

- V7 product content exposes three explicit fighter profiles, four weapon bases, two ultimates, and
  four ordinary passives. The player and Balance Lab no longer expose named build presets, point
  costs, a budget, Custom Pulse, or the two frame passives. Private legacy definitions may remain
  temporarily for direct-match diagnostics and historical regression coverage, but cannot enter
  saved-profile admission or the routed product handoff.
- Queue admission resolves the saved authored recipe into the existing immutable
  `ResolvedMatchLoadout`. Combat continues to consume resolved fighter stats, weapon, ultimate, and
  passives without account/profile queries.
- The V6 Balance Lab remains the server-authoritative tool for tuning the surviving fighter-profile
  and weapon-base definitions. M01 must update its schema/UI/resolution tests to remove preset,
  point-budget, Custom Pulse, and frame-passive assumptions while preserving accepted live Practice
  tuning behavior. This is compatibility work caused directly by the V7 content model, not a new
  Balance Lab feature.

### Dashboard behavior

- Replace the single local-build card/editor with a bounded brawler list, selected marker, and
  create/edit/delete actions. Creation clearly labels fighter profile and weapon base as permanent
  before confirmation.
- Editing exposes name, ultimate, and two passives. Permanent fields are visible but not editable.
  M02 adds the four part slots to this same screen rather than creating another build authority.
- All mutation controls show a pending state until the server outcome arrives. Rejection restores
  the last accepted snapshot and gives a short actionable reason.
- Joining Practice or matchmaking uses the selected brawler. Queueing is disabled when the profile
  is empty, loading, invalid under current content, or has a mutation in flight.
- While queued, create/edit/select/delete controls are disabled locally and still rejected by the
  server. Leaving queue restores them. Active-match presentation uses only the admitted snapshot.
- The complete flow remains keyboard, mouse, and controller accessible. Destructive deletion
  requires confirmation and identifies the brawler by display name without implying uniqueness.

### Backup and restore

- Add a focused server-side `brawler-profile-admin backup --database <path> --output <path>` command
  using SQLite's online backup API. It validates the source application/schema before writing,
  refuses to overwrite an existing output, writes to a temporary sibling, validates the completed
  copy, then atomically installs it. It does not require the lobby or supervisor to open profile
  data and does not become a generic database CLI.
- The first operator workflow may run while the lobby is stopped. The online backup API is still
  required so the artifact is correct for a WAL database and the command can later be coordinated
  with a live owner without changing its format.
- Restore remains an operator file operation while the lobby is stopped, documented as preserving
  the old database/WAL/SHM set before replacement. M01 tests restore a backup into a fresh data
  directory, restart the logical server, and compare IDs, revisions, selection, and brawlers.
- No automatic repair, truncation, deletion, or replacement occurs after a failed integrity check.

## Implementation plan

Implementation began on 2026-08-22. The product path now uses the saved-profile model, V2 immutable
handoff, lobby-owned SQLite authority, stable supervisor/client identities, and the saved-brawler
Dashboard as one vertical slice. Legacy transient-build definitions remain private compatibility
scaffolding for direct-match diagnostics and historical tests, but the product Dashboard, queue,
Practice/multiplayer admission, routing handoff, and Balance Lab do not expose or trust them. This
keeps M01 focused without making the V7 player flow depend on the obsolete preset/budget model.
The canonical local multi-client launcher assigns persistent numbered client-data slots so
simultaneous interactive clients never race on one connection/identity file.

### 1. Content and shared profile model

- [x] Reconcile against the accepted final V6 content and Balance Lab behavior.
- [x] Add stable `AccountId`, `SavedBrawlerId`, `FighterProfileId`, and `WeaponBaseId` types with
  bounded wire/storage codecs and redacted or safe diagnostics as appropriate.
- [x] Promote Default/Lightweight/Reinforced to creation choices; make the four current weapons
  canonical bases; remove named presets, point costs/budget, Custom Pulse, and frame passives from
  the player and Balance Lab surfaces.
- [x] Add bounded profile/saved-brawler validation and canonical loadout resolution.
- [x] Retire legacy local-build loading from the product flow without deleting its file.

### 2. Stable logical-server and client identity

- [x] Add validated supervisor data-directory configuration and atomic stable logical-server ID
  creation/loading.
- [x] Extend the lobby manifest/configuration with the lobby-only profile database path and bump its
  canonical manifest version/digest tests.
- [x] Advance the client connections schema with per-logical-server account bindings, address
  bootstrap/alias handling, bounded atomic persistence, and corruption preservation.

### 3. SQLite storage owner

- [x] Add server-only `rusqlite` with the reviewed bundled/backup features and verify the exact lock
  and toolchain compatibility before committing the dependency.
- [x] Implement the dedicated bounded storage thread, startup handshake, nonblocking command/result
  pumps, graceful shutdown, and fatal failure signaling.
- [x] Implement application ID, WAL/foreign-key configuration, schema v1 migration, integrity and
  semantic validation, transactional load-or-create and CRUD, and typed failures.
- [x] Implement the bounded backup command and restore verification harness.

### 4. Lobby profile authority and protocol

- [x] Add the pending-profile handshake phase, single-session-per-account rule, accepted cache,
  optimistic revisions, request idempotency, and transactional mutation coordination.
- [x] Register the reliable ordered profile channel/messages and bump the global protocol.
- [x] Replace client-authored queue builds with selected-brawler admission and atomic queue locking.
- [x] Define/install the V2 immutable match loadout snapshot and revise opaque routing participant
  metadata without exposing account identity.

### 5. Client Dashboard and lifecycle

- [x] Replace the preset/custom build editor with profile loading, empty-state creation, bounded
  brawler list, creation, edit, select, and confirmed deletion flows.
- [x] Add pending/rejection/queue-lock states and controller-accessible navigation.
- [x] Preserve selected brawler and profile convergence across lobby → match → lobby reconnect and
  client restart.

### 6. Balance Lab and documentation reconciliation

- [x] Adapt the accepted V6 Balance Lab schema, UI, tuning transaction, and tests to the surviving
  fighter-profile/weapon-base model.
- [x] Update product, weapon/build, UX, networking, server architecture, operator, README, and
  backlog documentation to the implemented contracts and commands.

## Implementation and verification evidence — 2026-08-22

- The lobby now delays welcome until the bounded storage executor returns the validated profile,
  rejects simultaneous use of one account, owns optimistic profile mutations, freezes the selected
  brawler at queue admission, and serializes only the V2 immutable loadout into routed match
  manifests. Match workers receive neither account identity nor storage configuration.
- The Dashboard starts an empty profile in explicit creation, labels the three fighter profiles and
  four weapon bases as permanent, and provides server-backed create, select, edit, and confirmed
  delete flows. Name, ultimate, and the two surviving passives remain editable; mutation and
  admission controls honor pending and queue-locked state.
- Client connection schema 2 stores one account binding per logical server and canonical address
  aliases. The native check exposed that RON cannot encode raw `u128`; account IDs now persist as
  the specified 32-character lowercase hexadecimal string, with a full-width-ID regression test.
- Balance Lab schema 2 tunes the three fighter profiles and four weapon bases without Custom Pulse,
  named presets, point-budget, or frame-passive surfaces. Its server validation re-resolves all
  twelve fighter/base combinations through the V7 saved-brawler resolver.
- SQLite startup verifies the exact application/schema IDs, full integrity, foreign keys, and
  semantic records. Wrong/newer/corrupt stores are preserved and rejected. The online backup
  command refuses overwrite, validates its result, and the restore test recovers the exact profile.
- `just check`, `just lint`, and the final `just test` rerun pass every role, warnings-as-errors
  Clippy, web build, feature isolation, 413 client tests, 330 server/Balance Lab tests, all 82
  network scenarios, all 14 performance gates, and the renderer-boundary check.
- Routed `just e2e 2`, `just e2e 4`, and `just e2e 6` each reached one exact Active 1v1, 2v2, and
  3v3 match respectively through the saved-profile admission path and shut workers down cleanly.
- Native 1280×720 verification created a Lightweight + Arc Launcher brawler, edited its name and
  abilities, reconnected with the same client identity, and recovered the exact saved brawler.
  Native 960×720 verification confirmed the compact empty-profile creation layout and disabled
  admission state remain readable and usable.

## Verification plan

### Focused model and storage tests

- ID wire/hex/SQLite round trips, zero/malformed rejection, normalized duplicate names, immutable
  creation fields, catalog-reference validation, and deterministic loadout fingerprints.
- Empty profile, atomic get-or-create, reconnect, create/edit/select/delete, first-selection and
  selected-deletion fallback, monotonic revisions, stale/duplicate request behavior, 16-brawler cap,
  and transaction rollback on every injected failure point.
- Schema 0→1 migration, repeated startup idempotence, expected application ID, newer/wrong database
  rejection, WAL/foreign-key configuration, integrity plus foreign-key checking, malformed-row
  rejection, and preservation of the original files.
- Bounded executor saturation, result backpressure, startup failure, runtime disconnect, panic
  propagation, and shutdown/join behavior without blocking Bevy schedules.
- Backup while WAL is configured, refusal to overwrite, backup validation, restore into a fresh data
  directory, and exact recovered profile equality.

### ECS and network integration tests

- Pending hello does not receive lobby acceptance before storage completion; malformed identity and
  unavailable storage reject/fail fast without partial session state.
- Two sessions using one account ID follow the accepted policy; a disconnected account can reconnect
  and recover its authoritative snapshot.
- Mutation success replaces the client snapshot; stale, invalid, oversized, queue-locked, and
  storage-failed commands preserve the last accepted server/client state.
- Queue admission racing an edit has one deterministic serialized outcome. Accepted admission
  freezes the exact selected brawler revision and later profile state cannot change the match.
- Separate-App and routed-process tests prove the match worker receives and executes the resolved
  fighter/weapon/ability loadout while having no account, profile authority, database path, or
  SQLite connection.
- Supervisor and lobby restart preserve logical-server ID and profiles. Client restart sends the
  same account ID for that logical server. Unknown well-formed deterministic test IDs create one
  profile idempotently.

### Canonical gates and manual checks

- Run the repository's existing `just check`, `just lint`, `just test`, and routed `just e2e 2`,
  `just e2e 4`, and `just e2e 6` gates after adding focused profile/backup coverage to those
  canonical paths.
- Prove the ordinary server has no development-only Balance Lab HTTP surface and the client build
  acquires no SQLite dependency. Prove match-worker runtime composition has no storage authority or
  database access.
- Native 1280×720 and a narrow supported Dashboard check: first creation, duplicate names, three
  profiles/four bases, edit, select, delete, controller navigation, reconnect, queue lock, Practice,
  and multiplayer.
- Operator check: backup an owned profile, stop the server, restore it into a fresh data directory,
  restart, and recover the same selected brawler.

## User playtest handoff

M01 entered user playtest on 2026-08-22 after its canonical gates, routed E2E matrix, and native
wide/compact checks passed. Start a fresh persistent two-client environment from the repository
root with:

```bash
BRAWLER_DEV_DATA_DIR=target/v7-m01-playtest just run 2
```

The two windows use stable client identities under `target/v7-m01-playtest/clients`, while the
logical server keeps its identity and profile database under `target/v7-m01-playtest/server`.
Stopping with Ctrl-C and running the same command again exercises recovery rather than creating a
new environment.

Use this bounded scenario:

1. Connect with no prior server identity and create two brawlers using different permanent profiles
   and weapon bases, including duplicate display names.
2. Edit/select them, restart client and server, and confirm the same arsenal returns.
3. Queue with one selected brawler and confirm edits are unavailable/rejected while queued.
4. Enter Practice and one multiplayer mode and verify the admitted movement/health, weapon,
   ultimate, and passives.
5. Delete the selected brawler and confirm deterministic fallback; delete all and confirm creation
   becomes the required next action.
6. Restore the operator backup and confirm the pre-deletion profile returns.

Requested observations: clarity of the permanent choice, speed of creating another brawler,
Dashboard/controller navigation, trust in save/pending/error feedback, and whether the three launch
profiles feel meaningfully distinct.

## Risks and controls

- **V6 overlap:** V7 invalidates parts of the accepted Balance Lab model. M01 updates that tool in
  the same content cutover and preserves its verified live Practice behavior.
- **Identity loss or collision:** IDs use OS-generated nonzero 128-bit values; local loss is clearly
  reported as unrecoverable in this insecure phase. No display name or address becomes ownership.
- **Lobby frame stalls:** all database access stays on the dedicated bounded thread and ECS uses
  nonblocking queues.
- **Queue/edit race:** one `ProfileAuthority` serializes mutations and admission before emitting the
  immutable match snapshot.
- **WAL backup mistakes:** only SQLite's backup API creates supported backups; raw database copying
  is explicitly unsupported.
- **Content drift:** saved authored selections are revalidated and re-resolved; invalid selections
  are reported for player repair instead of silently rewritten.
- **Architecture creep:** M01 remains one local SQLite owner and one profile protocol. Provider auth,
  web adapters, cloud storage, inventory, acquisition, and generic backend infrastructure remain
  deferred.

## Specification-review decisions

The user accepted these research recommendations by authorizing M01 implementation on 2026-08-22:

1. Launch with Default, Lightweight, and Reinforced as the three permanent fighter-profile choices,
   and remove Lightweight Frame/Reinforced Frame from swappable passives.
2. Start a new profile empty; open creation immediately, preselect safe defaults, and automatically
   select the first confirmed brawler.
3. After deleting the selected brawler, select the oldest remaining brawler; if none remain, return
   to the required creation state.
4. Reject a second simultaneous lobby session using the same insecure `AccountId`; allow reconnect
   after the first session has ended.
5. Provide the operator backup command in M01, while restore is a documented stopped-server file
   operation exercised by an automated and manual restore test rather than a production restore CLI.

## Exit criteria

- All five specification-review decisions have an accepted or revised recorded disposition.
- One stable logical server and one client identity recover the same validated profile after client,
  lobby, and supervisor restart; corrupt or unavailable storage never silently produces an empty
  replacement.
- Create/edit/select/delete enforce immutable creation fields, duplicate names, revisions, the
  16-brawler cap, transactional failure behavior, and the accepted simultaneous-session policy.
- The Dashboard fully replaces named presets, Custom Pulse, the point budget, and local build
  persistence for the player flow while preserving keyboard/mouse/controller access.
- Queue admission rejects concurrent/later edits, resolves the selected saved brawler once, and
  hands an immutable bounded snapshot to Practice and multiplayer match workers with no account or
  storage authority.
- SQLite application/schema identity, WAL, transactions, migrations, integrity/foreign-key checks,
  backup, tested restore, bounded executor lifecycle, failure preservation, and reporting pass.
- The accepted V6 Balance Lab continues to tune the remaining relevant fighter/weapon definitions
  without retaining obsolete player-build concepts.
- Canonical checks/tests/E2E, native visual checks, user feedback triage, and the learn-from-errors
  review pass before M01 is marked complete.
