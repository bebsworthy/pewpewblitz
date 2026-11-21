# Outcome
Player data survives losing the host: profiles are backed up off-host on a schedule, a restore has been rehearsed, and every schema change from the first public deploy onward migrates existing data instead of dropping it.

# Found (2026-11-21)
## Data directory (`--data-directory`, the container volume)
`profiles.sqlite3` with its `-wal` and `-shm` files (WAL mode), plus `logical-server-id`. Losing `logical-server-id` makes clients treat the server as a new logical server, and each would create a new account (`connection_persistence.rs:238`), so it must be backed up with the database.

## Schema (`src/profiles/storage.rs`)
- `application_id` 1112692556 and `user_version` 4 (`SCHEMA_VERSION`). Every open runs `migrate` then `validate_database`, which includes a full `integrity_check`.
- A newer schema than the binary is refused; schema 3 is retired by dropping tables "without migration (local test data)"; anything older is refused.
- Tables:
  - `profiles` (account key, revision, selected character).
  - `character_choices`.
  - `published_maps` (account key, `author_display_name`, map name, recipe ≤ 256 KiB, timestamps, moderation reason).
  - Account deletion cascades through foreign keys.

## Backup tool (`brawler-profile-admin backup`, `storage.rs:462`)
- Uses the SQLite online backup API into `<output>.tmp`, validates the copy, then renames it. It refuses existing outputs, which makes it safe against a live WAL database.
- **Risk:** it opens the source through `ProfileStorage::open`, which **runs `migrate`**. A newer admin binary pointed at a live database would migrate it underneath a running older lobby.
- No schedule, no off-host copy and no retention exist.

# Decisions (owner-approved 2026-11-21)
1. **Backup source opens read-only without migrating.** It refuses if `user_version` ≠ the tool's `SCHEMA_VERSION`, and it also copies `logical-server-id`, so the output is a directory: database plus id.
2. **Schedule:** a small backup service in the BRL-jjwa compose stack runs the admin backup every 6 h. That's a recovery point of at most 6 h of profile changes, which are only character picks and published maps.
3. **Off-host:** each backup is uploaded to a private Scaleway Object Storage bucket in a *different* region from the instance (S3 API; `rclone` in the backup service image), with a bucket lifecycle rule expiring objects after **30 days**. The 30 days is also the deletion window stated in BRL-tysh. Credentials are a compose secret.
4. **Migration policy from the first public deploy:**
   - Every schema change adds a forward `N → N+1` step in `migrate`, tested from a checked-in fixture database of version N.
   - Data is never dropped without migration, and downgrades stay refused.
   - Before migrating, the lobby writes a pre-migration backup beside the database.
   - The schema-3 retire path and the "delete this test database" refusal stay valid only before launch; the BRL-4xjh hashed-key change lands before the first public deploy on a fresh database.
5. **Restore procedure** in `docs/19-server-operations.md`: stop the stack, replace the volume contents from a chosen backup, start, and verify (lobby accepts and an existing account sees its profile).

# Scope
Decisions 1–5. A unit test for the read-only, no-migrate backup. A migration-fixture test harness, even if no migration exists yet. Pre-migration backup.

# Non-goals
Point-in-time recovery, continuous WAL shipping (Litestream-style), multi-host replication.

# Acceptance
- The backup tool against a live lobby database with a different `user_version` refuses and leaves it unchanged.
- The compose stack produces a backup within one interval, and it appears in the bucket.
- A restore drill on a fresh DEV1-M (or a local container) recovers an account's character pick and published map; the result is recorded here.

# Verification
`just test`, the backup unit tests, and the restore drill commands and results recorded here.
