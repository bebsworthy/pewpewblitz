# Outcome
The PewPew Blitz stack runs on the Scaleway host from a documented, repeatable setup, restarts by itself after a crash or reboot, and is deployed and operated with a few scripted commands.

# Inputs (decided)
- Host: Scaleway DEV1-M, x86_64 (BRL-kc93).
- Artifact: a container image (`linux/amd64`) plus a compose stack: supervisor with `tini`, `brawler-token-service`, Caddy, and the backup service (BRL-jjwa, BRL-z277, BRL-sbep, BRL-kdza).
- Health, logs and firewall come from BRL-71r3 and BRL-h018.
- Runbook home: `docs/19-server-operations.md`.

# Found (2026-11-21)
- **The lobby already restarts itself.** The supervisor restarts a crashed lobby worker; covered by `real_bevy_lobby_restarts_after_crash_and_cleans_exactly` (`tests/routed_process.rs:427`). A match-worker crash ends only that match (`real_bevy_workers_isolate_match_crash_and_cleanup_routes_peers`). The container restart policy only needs to cover a supervisor exit.
- **Data and secrets:** the volume holds `profiles.sqlite3` (WAL), `logical-server-id`, the status file (BRL-71r3) and failure records. Secrets are the Netcode key and the object-storage credentials.

# Decisions (owner-approved 2026-11-21)
1. **Host OS:** Scaleway's Ubuntu 24.04 LTS image; Docker Engine plus the compose plugin from Docker's apt repository; unattended security upgrades (BRL-h018). Provisioning is a checked-in `deploy/cloud-init.yaml` (or an idempotent `deploy/provision.sh`) that also creates the `brawler` system user, `/var/lib/brawler` (the volume) and `/etc/brawler/secrets` (0600, root-owned and mounted as compose secrets).
2. **Restart:** every service `restart: unless-stopped`; Docker enabled at boot; the compose stop grace period from BRL-pweq/BRL-sbep.
3. **Deploy:** `deploy/deploy.sh <image-tag>` runs over SSH: pull, drain (BRL-pweq), `docker compose up -d`, then wait for `/healthz` 200 and report. Rollback is the same script with the previous tag (images are tagged by SHA).
4. **Key generation:** `deploy/new-key.sh` writes a fresh 32-byte key to `/etc/brawler/secrets/netcode.key` once. Rotating it invalidates only in-flight tokens.
5. Everything lives in `deploy/` and is documented in `docs/19-server-operations.md`: provision, deploy, rollback, logs, backup and restore, incident handling.

# Scope
Decisions 1–5. Shell scripts pass `shellcheck` (add to `just lint` if available).

# Acceptance
- A fresh DEV1-M provisioned from the checked-in files serves the stack.
- `sudo reboot` brings it back with `/healthz` 200.
- Killing the supervisor container leads to an automatic restart.
- `deploy.sh` to a new tag and back works. All of it is recorded here.

# Verification
The provisioning and deploy run on the real host, with commands and output recorded.
