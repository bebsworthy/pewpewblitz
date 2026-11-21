# Found (2026-11-20 deployment survey)
The routed topology (one UDP port, supervisor, lobby worker, worker per match, SQLite WAL profiles, `brawler-profile-admin` backup and map moderation) is deployable in shape. Security, packaging, operations and release process around it are missing. Child tickets own each gap; the blockers are the shared Netcode key and unverified account identity.

# Acceptance
Every child is done or explicitly deferred, and a public host runs a server that external Android and desktop clients can join.

# Spec-preparation plan (2026-11-20)
A child's spec is ready for `todo` when it records the research findings, the decision taken, scope and non-goals, acceptance criteria, verification commands, the owning doc for durable behavior, and any playtest needs. Steps run in order; the work inside a step can run in parallel.

## Step 1: owner framing questions (on this ticket)
These change several specs at once, so ask them first:
1. Launch audience: closed test (friends; Play internal testing track) or open public. Sets how much of BRL-fygr, BRL-tysh and BRL-h018 is needed at launch.
2. Hosting budget and player region. Feeds BRL-kc93 and BRL-h018.
3. Release artifact: tarball plus systemd, or a container image. Feeds BRL-jjwa, BRL-n7c8 and BRL-mh5y.
4. Account recovery after reinstall or a new device: none at launch, or a recovery code. Feeds BRL-4xjh.
5. Owning doc for operations: a new `docs/19-server-operations.md`, or sections in docs/14.

## Step 2: research, read-only, findings recorded in each child spec
- **Auth design (BRL-z277 and BRL-4xjh together).** Lightyear 0.29 `lightyear_netcode/src/token.rs`, `Authentication::Token`, the client flow in `references/lightyear/book`. Map each token issue point: lobby connect, match handoff, rejoin, network-change continuity. Measure the effect on capability routes and the pre-auth limits in `limits.rs`. Write one design, shared by both tickets, into `docs/08-network-architecture.md`.
- **Process lifecycle (BRL-sbep and BRL-jjwa).** The `ctrlc` `termination` feature. What a worker does on control or packet IPC EOF. Behavior differences on Linux (Unix-socket paths, `TMPDIR`, child reaping). Which tests already cover shutdown.
- **Capacity (BRL-fdbz and BRL-kc93).** Measure worker RSS and CPU per mode with the existing metrics file and performance gates. Find the current queue-refusal path and what the client shows.
- **Compatibility UX (BRL-pweq).** The current fingerprint mismatch path from supervisor to client, and what the player sees. What a drain needs from the existing graceful-shutdown path.
- **Ops surfaces (BRL-71r3, BRL-h018 and BRL-mh5y).** Which flags and features must be refused in a release build. Health-signal options that don't open a second public surface.
- **Data (BRL-kdza and BRL-tysh).** Every table keyed by account. Where source addresses are logged. How backup retention interacts with deletion. A schema-migration approach for profile storage.
- **Client address (BRL-3he3).** How the Android prefill and the desktop address resolve today, and where DNS resolution would go.

## Step 3: write specs in dependency order
1. BRL-z277, then BRL-4xjh, then BRL-fygr (security chain; most design risk).
2. BRL-sbep and BRL-jjwa, then BRL-mh5y, then BRL-n7c8 (build-and-run chain).
3. BRL-fdbz, BRL-71r3, BRL-kdza (operations).
4. BRL-pweq and BRL-3he3 (release flow).
5. BRL-kc93, BRL-h018, BRL-tysh (decision and compliance tickets; mostly owner answers plus a checklist).

Any choice beyond existing direction gets its own question on the child ticket. Blocking questions stop only that child.

## Step 4: review and promote
Present each finished spec to the owner. Move it from `backlog` to `todo` on agreement. Implementation stays one ticket at a time.

# Owner decisions (2026-11-21)
- **Audience:** open public release on the Play Store, not a closed test (q-tzc8kxb). Moderation (BRL-fygr), privacy and account deletion (BRL-tysh) and UDP hardening (BRL-h018) are in launch scope.
- **Hosting:** Europe, small VPS at about €5–20 per month (Hetzner, OVH or Scaleway class, dedicated IPv4, UDP allowed) (q-twvv8jw).
- **Packaging:** container image with the data directory on a mounted volume, not tarball plus systemd (q-g1vceqa).
- **Account recovery:** none at launch; a reinstall starts a new account. Recovery is a later ticket (q-42qkqj0).
- **Operations doc:** new `docs/19-server-operations.md`, linked from `docs/README.md`. Topology stays in docs/14 and security in docs/08 (q-g5fpzcg).

# Spec preparation complete (2026-11-21)
All 16 children have owner-approved specs and are in `todo`, in this board order: z277, 4xjh, sbep, jjwa, fdbz, pweq, kdza, tysh, mh5y, 71r3, h018, fygr, d8pv, 3he3, n7c8, kc93.

**Open owner questions:**
- Domain name (BRL-3he3, q-6htbvp0, blocking for the release default and the TLS certificate).
- Scaleway zone (BRL-kc93, q-hgnvryd).
- Data controller and contact (BRL-tysh, q-z8t68zd, blocking the policy text only).

**Deferred:** player reports, BRL-f4vh (idea).

**Evidence attached:** capacity script and samples on BRL-fdbz; Hetzner availability screenshot on BRL-kc93.
