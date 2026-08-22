# Version 7 implementation roadmap

## Purpose and scope

V7 promotes the persistent arsenal into a player-visible product loop. A player has one stable
server-side profile, creates saved brawlers, permanently chooses each brawler's fighter profile and
one of the four established weapon bases, equips up to four interchangeable weapon-part instances,
and recovers the same accepted arsenal after client, profile-owner, or logical-server restart. A
server-side profile authority validates ownership and resolves the selected brawler into the
immutable loadout handed to an isolated match worker. In V7 that authority lives in the long-lived
lobby worker and uses a dedicated bounded storage executor as the exclusive SQLite owner. Combat
remains unaware of accounts, storage, acquisition history, part names, and inventory presentation.

V7 provides only the identity, persistence, starter inventory, editing, resolution, recovery, and
match-handoff behavior required by that loop. It does not commit production/global authentication,
cloud or cross-server profiles, progression, currency, rewards, random acquisition, loot boxes,
shops, purchases, trading, crafting, rarity-driven power, social systems, or a general backend
platform.

## Version status

| Field | Value |
|---|---|
| Status | User playtest |
| Current milestone | M01 — Durable profile and saved-brawler loop |
| Entry gate | Satisfied: V6 completed and was accepted on 2026-08-22; the user authorized M01 implementation and accepted its five specification-review recommendations on 2026-08-22 |
| Completion gate | A server-owned profile can create and recover a saved brawler with an immutable fighter-profile/weapon-base pair, equip four interchangeable owned parts, enter routed Practice and multiplayer with the correctly resolved immutable loadout, and preserve authority and storage integrity across restart and failure checks |

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Product decisions already accepted

- A saved brawler permanently binds one server-known fighter profile and one weapon base at creation.
  Its name, equipped parts, ultimate, and two passives remain editable outside queue; the fighter
  profile and weapon base do not.
- An account may own at most 16 saved brawlers. Brawler names are presentation metadata and do not
  need to be unique. A player may delete a saved brawler.
- V7 starts with a fresh server-side arsenal and does not import today's locally saved build.
- V7 uses an explicitly insecure development identity seam. A client or test supplies an opaque
  `AccountId`; the server validates its bounded format, then atomically loads the matching profile or
  creates it when absent. A normal client generates and stores one ID per logical server, while tests
  may use stable deterministic IDs. There is no proof-of-possession, production security check, or
  profile-creation rate limit. Changing or losing the local ID selects or creates another account,
  and there is no recovery.
- Queue admission freezes the accepted brawler. Every brawler edit is rejected while that brawler's
  player is queued; an active match continues to use its immutable admitted snapshot.
- Ultimates and both passive selections remain freely swappable outside queue. V7 removes the
  12-point build budget rather than carrying it into the persistent arsenal.
- The named Runner, Bruiser, Controller, and Duelist builds disappear from the player product. They
  do not become starter templates; creating another brawler is the quick way to try another build.
- The four current reference weapons are the initial weapon bases. Additional bases remain ordinary
  complete validated configurations added through later content work.
- Every weapon has exactly four interchangeable part slots. Slots have no type, family, or gameplay
  role, and rearranging the same parts cannot change resolved behavior or its gameplay fingerprint.
- Part type, generated name, icon, and model are presentation/inventory metadata. Gameplay comes
  only from bounded typed effects stored and validated as server-owned part-instance data. V7 does
  not render equipped parts on the in-match fighter or weapon.
- Empty part slots are legal. Distinct owned instances of the same part definition may be equipped
  together, but one instance may not occupy multiple slots. Numerical modifiers sum flat values,
  then apply the combined percentage, then clamp and round once; repeated statuses aggregate into
  one bounded effect per status kind.
- Effects are broadly compatible with weapon bases, but an effect that cannot apply to a base makes
  the edit invalid instead of being silently ignored. Parts should be readable sidegrades with
  bounded tradeoffs rather than uncapped power upgrades.
- Acquisition source does not affect combat. V7 may seed a bounded starter inventory but does not
  implement the future reward, purchase, loot, or trade mechanisms that grant parts. The starter
  inventory is fixed authored content, the initial account inventory cap is 128 part instances, and
  persisted generated rolls are never silently rerolled by a balance update.
- Initial Frost parts use the existing bounded slow effect. Accumulating Frost remains a later
  target-owned status capability.
- Weapon bases plus part effects resolve through the existing `WeaponConfiguration` validator into
  the existing immutable `ResolvedWeapon`; match combat never queries inventory or storage.
- SQLite with WAL and transactions is the accepted first persistence baseline. V7 also requires
  versioned migrations, an operator backup command, and a tested restore path before completion.
- Storage unavailability fails fast. On corruption, the service preserves the database for recovery,
  rejects unsafe records, reports the fault, and never silently resets owned data.
- The long-lived lobby worker owns the logical-server-local `ProfileAuthority`. Its ECS systems send
  bounded commands to a dedicated storage executor/thread that exclusively owns SQLite and returns
  bounded results without blocking lobby schedules. The supervisor supplies storage configuration
  but never opens profile data; match workers receive immutable snapshots without store access.
- Persistent account identity, saved-brawler identity, part-instance identity, match `PlayerId`,
  replicated `NetworkEntityId`, routed identity, and process-local Bevy `Entity` remain distinct.
- M01 research recommends promoting the existing Default, Lightweight, and Reinforced stat profiles
  into three permanent creation choices on the same fighter body, while removing the two equivalent
  frame passives. This recommendation remains part of specification review.

## Milestone overview

| Milestone | Status | Player-visible deliverable | Plan |
|---|---|---|---|
| 01 | User playtest | Minimum server-side profile and durable arsenal: create a saved brawler with a permanent fighter-profile/weapon-base pair, select it from the Dashboard, recover it after restart, and enter matches through the existing routed handoff | [Milestone 01](./milestone-01.md) |
| 02 | Not started | Four-slot weapon-part equipment: receive a bounded starter inventory, equip any four legal owned parts in interchangeable slots, preview the result, persist it, and play the resolved weapon through the existing combat path | Planned boundary: add the first inventory/equipment migration and typed part resolution only after M01 persistence, queue, Dashboard, and match-handoff evidence; create the milestone file then |

## V7 architecture selected by M01 research

```text
client intent + AccountId
  -> long-lived lobby worker
  -> ProfileAuthority application transaction
  -> bounded storage command/result
  -> dedicated SQLite storage executor
  -> selected saved-brawler revision
  -> canonical server resolution
  -> immutable queue/reservation/match-manifest snapshot
  -> isolated match worker and existing combat runtime
```

The placement is accepted: profiles are local to one logical server, `ProfileAuthority` lives in the
long-lived lobby worker, and its dedicated storage executor exclusively owns SQLite. M01 research
selected a synchronous `rusqlite` connection on one bounded dedicated thread, embedded forward-only
migrations, SQLite's online backup API, fail-fast startup, and immutable match handoff as the smallest
safe implementation. The supervisor provides the database path and stable logical-server identity
but does not open the store. A generic repository framework or separate profile microservice is not
needed.

The M01 specification additionally covers:

- stable ID format and profile-key handling;
- atomic get-or-create behavior, per-logical-server client storage, malformed ID rejection, stable
  deterministic test profiles, and the policy for two simultaneous sessions presenting the same
  insecure ID;
- optimistic revision checks and idempotent create/equip/select requests;
- deletion integrity, the 16-brawler and 128-part caps, non-unique names, and immutable creation
  fields;
- schema versions, migrations, corruption preservation, atomicity, backup/restore, and owner-process
  restart;
- queue-time edit rejection and snapshot freezing;
- result/reward boundaries without implementing progression or acquisition;
- fail-fast unavailable-storage behavior and controller-accessible Dashboard flows;
- isolation proving that the client feature graph and supervisor do not acquire SQLite, and that
  match-worker runtime composition receives no database configuration, connection, profile cache,
  or account authority;
- the launch fighter-profile catalog and presentation of permanent creation choices.

## Initial V7 backlog

| ID | Item | Disposition |
|---|---|---|
| V7-PRODUCTION-AUTH | Provider adapters for Steam, Apple/Game Center, Google Play, and standalone identity; internal account sessions; explicit identity linking; credential recovery, rotation, revocation, rate limiting, and production security | Deferred; V7 uses only its clearly labeled caller-supplied development `AccountId` seam. A future scheme maps verified provider identities to that stable internal ID rather than changing profile ownership |
| V7-DEVICE-POSSESSION | Proof that a reconnecting standalone client still possesses its enrolled device credential | Deferred. If needed before provider authentication, prefer a nonce-bound signature or HMAC challenge over TOTP; TOTP adds clocks and replay windows without improving first-contact trust |
| V7-CLOUD-PROFILES | Cross-server or cross-device profiles and shared database deployment | Deferred until a supported hosting topology requires it |
| V7-WEB-ARSENAL | Browser profile/dashboard using an HTTP adapter over the same profile authority and queue-edit rules | Deferred to [`CAND-WEB-ARSENAL`](../../backlog.md); it is independent of a browser gameplay client and never accesses SQLite directly |
| V7-WASM-CLIENT | Full Bevy WASM gameplay client using browser-compatible Lightyear transport and routed server ingress | Deferred to [`CAND-WEB-GAME-CLIENT`](../../backlog.md); browsers cannot use Brawler's current raw-UDP endpoint |
| V7-ACQUISITION | Levels, rewards, random generation, loot boxes, purchases, and shops | Deferred; M02 may seed starter instances only |
| V7-TRADING | Ownership transfer, listings, escrow, fraud controls, and audit policy | Deferred until trading becomes a selected product outcome |
| V7-CURRENCY | Earned or purchased currencies and monetization | Deferred |
| V7-CRAFTING | Crafting, dismantling, upgrading, rerolling, and affix mutation | Deferred |
| V7-ADVANCED-PART-EFFECTS | New structural delivery families or accumulating Frost/status behavior | Deferred to a separately validated combat-capability slice; initial parts use implemented weapon properties/effects |
| V7-MULTIPLE-WEAPON-MODELS | Complete composited weapon/part attachment rendering | Presentation may begin with bounded inventory icons/models and existing combat readability; richer assembly art requires its own evidence |

## Version exit conditions

- The accepted profile/storage design is documented with explicit trust, failure, migration, and
  recovery boundaries.
- Fighter profile and weapon base are immutable saved-brawler creation facts; no ordinary mutation
  request can replace either.
- Accounts enforce the 16-brawler limit, permit deletion and duplicate display names, and start
  fresh without importing the legacy local build.
- A valid caller-supplied account ID atomically loads or creates its profile, including deterministic
  test IDs. Verification proves malformed IDs are rejected, no profile-creation rate limit is
  implied, the seam is clearly identified as insecure, and the ID is not confused with Netcode,
  routing, lobby, match, or display identity.
- Names, parts, ultimate, and both passives can be changed outside queue without a point budget.
  Runner, Bruiser, Controller, and Duelist are no longer player-facing builds or templates.
- Four generic slots accept four unique owned part instances without slot-type gameplay rules, and
  slot permutation leaves the resolved recipe fingerprint unchanged.
- Invalid ownership, stale revisions, duplicate instances, unsupported effects, malformed or
  oversized data, and storage failures change no accepted arsenal state.
- Queue admission freezes one canonical selected-brawler revision and resolved loadout. Every edit
  while queued is rejected, and no later edit can mutate an active match.
- Match workers execute the existing server-authoritative weapon runtime from the resolved loadout
  and do not query account, inventory, persistence, rarity, name, or acquisition state.
- Restart and recovery verification covers client, profile owner, and logical-server lifecycles.
  Unavailable storage fails fast; corruption preserves the database, rejects unsafe records, reports
  the fault, and never silently resets or duplicates owned data.
- Versioned migrations, SQLite WAL/transactions, an operator backup command, and a tested restore
  pass before V7 completion.
- Canonical role checks, unit/integration/network/process tests, routed Practice and multiplayer
  E2E, native Dashboard/arsenal checks, user feedback triage, and learn-from-errors review pass.
