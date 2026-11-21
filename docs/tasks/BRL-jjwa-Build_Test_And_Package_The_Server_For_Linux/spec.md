# Outcome
A commit on `main` produces a tested Linux container image of the server, published to a registry, that starts a working routed server from a mounted data volume and an operator key.

# Found (2026-11-21)
- CI (`.github/workflows/ci.yml`) runs only on `macos-14`, and no Linux build has been verified. Docker 29.8 (linux/aarch64) is available on the dev machine; no Rust image is cached.
- **Runtime inputs are small.** All gameplay catalogs and maps are compiled in with `include_str!` (`src/characters/definitions.rs:94`, `src/map/catalog.rs:652-662`, and others). At runtime the server needs only:
  - the `brawler-supervisor`, `brawler-server` (worker) and `brawler-profile-admin` binaries;
  - `config/server/game-types.ron`;
  - the data directory, holding `profiles.sqlite3` (WAL) and the persisted logical server id (`supervisor.rs`);
  - after BRL-z277, the token-service binary and the key.
- **Supervisor command line** (`supervisor.rs:389-450`): `--bind`, `--network-protocol`, `--protocol-registry-fingerprint`, `--content-fingerprint` (all printed by `brawler-server routing-identity`, as `scripts/dev.sh` does), `--worker-executable`, `--game-types`, `--data-directory`, optional `--metrics-file`. Release images must not pass `--match-rules verification`, `--mode` overrides or `--automatic-transition-driver` (BRL-mh5y).
- **IPC is Unix sockets** in a private 0700 directory under `std::env::temp_dir()` (`packages/brawler-routing/src/ipc.rs:525`, `:543`). Linux's 108-byte `sun_path` is roomier than macOS. The container must give a writable `/tmp`, a tmpfs if the root filesystem is read-only.
- **Native dependencies:** `rusqlite` is `bundled` and needs a C compiler at build time only. The server feature graph excludes rendering, windowing and audio crates (`scripts/check-server-features.sh:15`), so no system graphics or audio libraries should be needed at runtime. Unverified on Linux until the first build.
- The network test target needs `brawler-client` (the client graph, with ALSA and udev on Linux), so it doesn't belong in the Linux server job.

# Decisions
- **Artifact:** a container image (owner, q-g1vceqa).
- **Stack (from BRL-z277):** a compose file with three services. The supervisor (with its workers) publishes UDP 5000. `brawler-token-service` listens on the private network only. Caddy terminates TLS and publishes TCP 443. All share the operator key secret.
- **Image (owner-approved 2026-11-21):**
  - A multi-stage Dockerfile: build stage `rust:1.95-bookworm`, runtime stage `debian:bookworm-slim` (glibc, `ca-certificates`), non-root user, `tini` as init (BRL-sbep).
  - An entrypoint script that derives the routing identity with `brawler-server routing-identity` and execs the supervisor, mirroring `scripts/dev.sh supervisor_args`, so fingerprints can't drift from the binary.
  - Data at `/var/lib/brawler` (volume), config at `/etc/brawler/game-types.ron` (baked in, overridable).
- **Architecture (owner-approved 2026-11-21):** build for the BRL-kc93 host. Hetzner's cheapest class is arm64 (CAX) and others are amd64, so the CI build uses `docker buildx` for `linux/amd64` and `linux/arm64` unless BRL-kc93 settles one first.
- **Registry (owner-approved 2026-11-21):** GitHub Container Registry (`ghcr.io`), pushed from CI on `main` and tags, tagged with the commit SHA and the network protocol version.

# Scope
1. `Dockerfile`, `.dockerignore` (exclude `target/`, `references/`, `asset_src/`, `external_assets/`, `assets/`, `data/`), entrypoint script, and `deploy/compose.yaml` with the three services, the volume, the secret and `init: true`. The token service and Caddy wiring land fully with BRL-z277; until then the compose file has the supervisor alone.
2. A Linux CI job (`ubuntu-24.04`): server-role Clippy and tests, `brawler-routing` tests, `tests/routed_process.rs`, `scripts/check-server-features.sh`, then an image build. Push only on `main` and tags.
3. A smoke test in CI: run the image with a temporary volume, wait for `brawler-supervisor listening on`, and connect a headless client lobby probe if one runs without the client graph. Otherwise use the UDP default-route reply, and record which.
4. README "Deploying the server" pointer, and the full procedure in `docs/19-server-operations.md`.

# Non-goals
Host provisioning (BRL-n7c8, BRL-kc93); monitoring (BRL-71r3); backups (BRL-kdza); the release-flag gate itself (BRL-mh5y).

# Acceptance
- The Linux CI job is green.
- The image builds for the chosen architectures and starts, then stops cleanly on `docker stop` (after BRL-sbep).
- `docker run` with an empty volume creates `profiles.sqlite3` and the logical server id, owned by the non-root user.
- The image contains no Balance Lab assets, `tools/balance-lab-web` output or client binaries.

# Verification
The CI run link, a local `docker build` plus `docker run` on the dev machine (arm64), and image size and contents (`docker image inspect`, a file listing), recorded here.

# Consequence of BRL-kc93 (2026-11-21)
The host is a Scaleway DEV1-M (x86_64). The image targets **`linux/amd64` only**; the arm64 variant is dropped unless a later host needs it. CI builds amd64 natively on the Ubuntu runner; local builds on the Apple-silicon Mac use `docker buildx --platform linux/amd64` (emulated, slower) or rely on CI.
