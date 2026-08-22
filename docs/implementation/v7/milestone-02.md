# V7 Milestone 02 — Four-slot weapon-part equipment

## Status

`User playtest`

Research and specification planning were authorized on 2026-08-22 after the user accepted the M01
saved-brawler recovery loop. The user authorized implementation on 2026-08-22, accepting the four
review recommendations below.

## Player-visible outcome

Every profile receives a fixed starter inventory of eight weapon-part instances. From the
Dashboard, the player can equip up to four legal owned instances in four interchangeable slots,
preview the exact resulting weapon, save the equipment, recover it after restart, and use that
resolved weapon in routed Practice and multiplayer.

## Accepted product boundary

See the [V7 roadmap](./roadmap.md#product-decisions-already-accepted). This milestone implements
fixed starter inventory, equipment, resolution, persistence, preview, and immutable match handoff.
It does not add random acquisition, progression, rewards, currency, shops, purchases, trading,
crafting, upgrades, rerolls, rarity power, accumulating Frost, or in-match part visualization.

## Research findings

### Current Brawler implementation

- [`src/profiles/model.rs`](../../../src/profiles/model.rs) already owns bounded whole-profile
  snapshots, optimistic profile/brawler revisions, server-generated brawler identity, and the V2
  immutable match snapshot. A part mutation can extend this one transaction protocol; another
  inventory service or replicated ECS inventory is not justified.
- [`src/profiles/storage.rs`](../../../src/profiles/storage.rs) has schema v1, one exclusive
  `rusqlite` connection, transactional mutations, semantic load validation, WAL, integrity checks,
  and online backup. M02 should add one forward migration and retain this owner rather than create a
  second connection or storage layer.
- [`src/profiles/authority.rs`](../../../src/profiles/authority.rs) serializes one pending mutation
  per account and rejects every profile mutation while its session is queued. Full four-slot
  replacement fits its current idempotency, stale-revision, and queue-freeze behavior.
- [`src/client/profile.rs`](../../../src/client/profile.rs) mirrors only accepted whole snapshots
  and has one pending command. The equipment editor can keep a local candidate while the server
  continues to own acceptance; no optimistic inventory mutation is needed.
- [`src/client/flow.rs`](../../../src/client/flow.rs) already has a controller-accessible brawler
  editor and responsive Dashboard composition. Equipment belongs in that flow as one focused
  overlay/draft, not in the retired Custom Pulse editor.
- [`src/combat/definitions/mod.rs`](../../../src/combat/definitions/mod.rs) models complete
  `WeaponConfiguration` values for the four bases and validates code-owned/policy bounds for
  economy, cadence, delivery, payloads, status effects, and resolved byte size.
  [`src/combat/definitions/resolver.rs`](../../../src/combat/definitions/resolver.rs) then
  normalizes and fingerprints the immutable `ResolvedWeapon`. Parts should transform a cloned base
  configuration and finish through that validator instead of adding a combat-side modifier path.
- All four current bases have capacity, refill/recharge, fire cooldown, reach, and hostile damage.
  Their delivery families differ: Straight uses range and derived lifetime, Lobbed uses distance,
  and MeleeArc uses reach. A semantic `Reach` modifier can map across all three; a projectile-speed
  part cannot, so M02 does not seed one.
- The Arc Launcher already has one `Slow` using `StrongestRefreshes`; other bases have room for one
  additional payload effect. Frost must merge into one bounded hostile Slow per payload bundle so
  it neither duplicates status kinds nor exceeds the payload-effect limit.
- [`src/content.rs`](../../../src/content.rs) fingerprints weapon, map, and build catalogs for the
  global compatibility handshake. The weapon-part catalog must join that envelope; there is no
  separate part protocol version or compatibility decoder.
- [`src/protocol.rs`](../../../src/protocol.rs) already registers profile commands/outcomes on an
  ordered reliable channel. Extending those bounded messages is sufficient. Part inventory is
  application state and must not become a replicated component.
- [`packages/brawler-routing/src/limits.rs`](../../../packages/brawler-routing/src/limits.rs) caps
  each opaque match-build snapshot at 255 bytes. Serializing inventory instances or a worst-case
  `ResolvedWeapon` would violate the boundary. A fixed-size canonical modifier accumulator is
  compact enough to re-resolve the weapon in the match worker and verify its admitted fingerprint.
- The accepted Balance Lab owns live base tuning. M02 must make its validation and bot/human
  snapshots part-aware, but a part-tuning web surface is not required for this player slice.

### Local Bevy and Lightyear references

- [`references/bevy/examples/README.md`](../../../references/bevy/examples/README.md),
  [`ecs/message.rs`](../../../references/bevy/examples/ecs/message.rs), and
  [`ecs/change_detection.rs`](../../../references/bevy/examples/ecs/change_detection.rs) confirm
  Bevy's resource/message composition and explicit ordered-system pattern. M02 should extend the
  existing profile resources and ordered pumps rather than attach persistent inventory to gameplay
  entities.
- [`references/bevy/examples/ui/navigation/directional_navigation.rs`](../../../references/bevy/examples/ui/navigation/directional_navigation.rs)
  confirms controller focus can follow dynamic layouts. Brawler already owns its navigation
  abstraction, so the equipment editor should extend that production abstraction instead of
  importing the newer snapshot's exact API into Bevy 0.19.
- [`references/lightyear/book/src/concepts/replication/protocol.md`](../../../references/lightyear/book/src/concepts/replication/protocol.md)
  distinguishes application messages from replicated components and requires shared registration.
  [`concepts/transport/serialization.md`](../../../references/lightyear/book/src/concepts/transport/serialization.md)
  reinforces early bounded message serialization. Ordered reliable profile messages remain the
  appropriate transport.
- [`references/lightyear/examples/lobby/src/protocol.rs`](../../../references/lightyear/examples/lobby/src/protocol.rs)
  demonstrates shared lobby messages and components, but Brawler's server-owned arsenal is a
  request/outcome transaction, not continuously replicated lobby state. No prediction,
  interpolation, input protocol, or gameplay replication change is warranted.

### SQLite and dependency research

- SQLite supports `ALTER TABLE ... ADD COLUMN` with a non-null constant default and transactional
  creation of new related tables. That is sufficient to migrate existing v1 profiles without a
  table rebuild. See the official [ALTER TABLE documentation](https://www.sqlite.org/lang_altertable.html).
- SQLite explicit transactions keep the starter grant marker, part rows, equipment rows, and
  revision advance atomic. Brawler still has one writer, so a pool or a second database owner would
  add no value. See the official [transaction documentation](https://www.sqlite.org/lang_transaction.html).
- Composite foreign keys and uniqueness constraints can enforce account ownership, four slot
  indexes, and instance exclusivity. `foreign_key_check` remains necessary after migration and on
  startup. See the official [foreign-key documentation](https://www.sqlite.org/foreignkeys.html).
- The pinned `rusqlite` 0.40.2 backup API copies the migrated database through distinct source and
  destination connections. The M01 operator command and restore path therefore remain valid for
  schema v2. See the current [`rusqlite::backup` documentation](https://docs.rs/rusqlite/latest/rusqlite/backup/).

## Decisions from research requiring review

### 1. One instance is equipped in at most one profile slot

Interpret the accepted rule “one instance may not occupy multiple slots” across the whole profile,
not only within one brawler. An owned physical instance can therefore be installed on one saved
brawler at a time. The editor labels where an unavailable instance is equipped; the player must
unequip it before installing it elsewhere. This keeps ownership literal and lets SQLite enforce the
rule with one unique constraint. Distinct instances of the same definition remain legal together.

### 2. Seed eight broadly compatible sidegrades

Seed eight authored instances so four slots still require a real selection. The initial structural
set is:

| Part | Presentation type | Effects |
|---|---|---|
| Expanded Feed | Magazine | capacity `+2`; refill/recharge interval `+20%` |
| Quick Loader | Magazine | refill/recharge interval `-20%`; capacity `-1` |
| Heavy Payload | Ammunition | damage `+15%`; fire interval `+15%` |
| Light Payload | Ammunition | fire interval `-15%`; damage `-12%` |
| Long Assembly | Barrel | reach `+18%`; fire interval `+8%` |
| Compact Assembly | Barrel | fire interval `-10%`; reach `-15%` |
| Frosting Module | Element | hostile Slow of `15%` for 36 ticks; damage `-10%` |
| Overcharged Chamber | Chamber | damage `+4` flat; capacity `-1` |

“Magazine”, “Ammunition”, and the other type labels are inventory presentation only. Every starter
part and every combination of up to four distinct starter parts must resolve on all four launch
bases. These values are the first playable balance, not a rarity or progression scale.

### 3. Persist exact gameplay rolls on each instance

Each instance stores its exact typed effects and display name in the profile database. Its
definition ID points to presentation/starter provenance but does not cause the effects to be
reconstructed on load. A later catalog balance update changes future grants only; it never silently
rerolls an owned instance. This adds a small bounded effect payload today but avoids an incompatible
persistence rewrite when random generation arrives.

### 4. Keep the 255-byte match snapshot boundary

Queue admission aggregates equipped effects into a compact `CanonicalWeaponModifiers`, resolves the
weapon, and records its fingerprint in a V3 match snapshot. The match worker re-resolves the same
base plus canonical modifiers and rejects a fingerprint mismatch. Instance IDs, names, definition
IDs, slot order, and acquisition provenance do not cross the match boundary. Implementation must
prove the maximum encoded V3 snapshot remains at or below 255 bytes instead of raising the routing
limit pre-emptively.

## Technical specification

### Authored part content and stable identities

Add a shared `weapon_parts` concern and one embedded catalog, for example:

```text
content/v7/weapon-parts.ron
src/weapon_parts/
  mod.rs          composition and intentional shared API
  model.rs        stable definition ID, typed effects, canonical modifiers
  definitions.rs  authored catalog, starter set, validation, fingerprint material
  resolver.rs     pure aggregation and WeaponConfiguration transformation
  tests.rs        catalog, arithmetic, permutation, compatibility, and bounds
```

- `WeaponPartDefinitionId(u16)` is a stable authored content ID.
- `WeaponPartInstanceId` is a nonzero opaque server-generated 128-bit ID with the same bounded
  byte/hex behavior as `SavedBrawlerId`. It never aliases a definition ID.
- A `WeaponPartDefinition` has stable ID/key, bounded display name, bounded presentation type, and
  exact starter effects. Type, name, future icon, and future model never enter gameplay
  fingerprints.
- A profile-owned `WeaponPartInstance` has instance ID, stable inventory ordinal, definition ID,
  bounded persisted display name, one to four exact typed effects, and no rarity, level, price,
  source, or mutable roll state.
- The part catalog has its own schema/fingerprint format and joins the global gameplay-content
  envelope. IDs are sorted and append-only for V7. Unknown definitions or unknown effect variants
  reject unsafe records rather than rewriting or discarding them.
- `MAX_WEAPON_PARTS_PER_PROFILE` is 128, `WEAPON_PART_SLOT_COUNT` is 4, and the instance/effect/name
  codecs have explicit byte/count/scalar bounds. The maximum profile fixture must fit the reviewed
  whole-snapshot and lobby-welcome limits; raise those limits once only if measured encoded data
  proves the current 16/32 KiB bounds insufficient.

### Typed effects and deterministic arithmetic

M02 supports only properties demonstrated by all four launch bases:

```text
Capacity       { flat: i8,  percent_basis_points: i16 }
Damage         { flat: i16, percent_basis_points: i16 }
FireInterval   { flat_ticks: i16, percent_basis_points: i16 }
RefillInterval { flat_ticks: i16, percent_basis_points: i16 }
Reach          { flat_milliunits: i32, percent_basis_points: i16 }
Slow           { penalty_basis_points: u16, duration_ticks: u16 }
```

- Definition and instance validation rejects zero/no-op effects, duplicate property variants inside
  one part, non-finite conversions, out-of-range percentages, oversized flat values, unsupported
  status kinds, and serialized payloads over their bound.
- Across equipped parts, sum all flat values and all percentage basis points per property. Compute
  `(base + combined_flat) * (10_000 + combined_percent) / 10_000`, then clamp to the existing
  engine/catalog limit and round once. Integer division uses one named deterministic nearest-round
  rule with tested tie behavior; reach is calculated in milliunits and converted to `f32` only after
  its single final quantization.
- `Capacity` maps to magazine rounds or charges. `RefillInterval` maps to magazine refill or charge
  recharge. Lower `FireInterval`/`RefillInterval` values are faster; UI text translates this into
  player language without changing the stored sign.
- `Reach` maps to Straight range, Lobbed distance, or MeleeArc reach. After Straight reach changes,
  recompute lifetime once from final range and speed at the simulation tick rate before validation.
- `Damage` modifies each hostile damage effect in the base recipe. It is invalid if no applicable
  hostile damage exists; M02 never silently ignores an effect.
- Slow strength is expressed as a movement penalty. Sum part penalties with an existing base Slow,
  clamp the total penalty at 60%, take the maximum contributed duration, and emit exactly one
  hostile `Slow { StrongestRefreshes }` per applicable payload bundle. Adding Slow is invalid if no
  hostile damage bundle exists or the normalized bundle would exceed its effect-count bound.
- Aggregate effects without reading slot indexes. Canonical modifiers have a fixed field order, and
  the final `ResolvedWeapon` fingerprint comes from the existing normalized recipe. Slot
  permutation, instance identity, definition metadata, and inventory order cannot affect behavior
  or fingerprint.
- The pure resolver clones the selected base `WeaponConfiguration`, applies canonical modifiers,
  repairs only derived Straight lifetime, and calls the existing policy-aware configuration
  resolver. Combat receives only the resulting `ResolvedWeapon`.

### Saved profile and equipment model

Extend the accepted authored profile shapes:

```text
ProfileSnapshot
  ...M01 fields
  inventory: Vec<WeaponPartInstance>             # stable ordinal order, <=128

SavedBrawler
  ...M01 fields
  equipped_part_ids: [Option<WeaponPartInstanceId>; 4]
```

- Empty slots are legal. Occupied IDs must be owned by the same account, pairwise distinct in the
  brawler, and—subject to recommendation 1—not equipped in another brawler.
- Brawler creation starts with four empty slots. Deleting a brawler removes its equipment rows but
  retains every owned instance. Selecting or editing name/abilities does not alter equipment.
- `EquipWeaponParts` carries request ID, expected profile revision, brawler ID, expected brawler
  revision, and the full candidate four-slot array. One accepted transaction replaces all four
  slots and advances both profile and target-brawler revisions once. A byte-identical accepted
  retry follows M01 request idempotency; stale or conflicting requests change nothing.
- Add typed decisions for missing part, unowned part, duplicate/already-equipped instance, invalid
  part effect, and incompatible resolved weapon. The UI may group these into concise explanations,
  but storage faults and stale revisions remain distinct.
- `ProfileSnapshot::validate_bounded` checks IDs, inventory count/order/uniqueness, effect payloads,
  equipment references/exclusivity, and total encoded size. Catalog-aware semantic validation also
  resolves every equipped brawler before the authority accepts a loaded snapshot.

### SQLite schema v2 and starter seeding

Migration `1 -> 2` runs in the existing forward-only transaction:

```text
ALTER TABLE profiles ADD COLUMN
  starter_part_set INTEGER NOT NULL DEFAULT 0 CHECK(starter_part_set >= 0);

CREATE TABLE weapon_part_instances(
  account_id BLOB(16) NOT NULL,
  part_instance_id BLOB(16) NOT NULL,
  inventory_ordinal INTEGER NOT NULL CHECK(inventory_ordinal > 0),
  definition_id INTEGER NOT NULL,
  display_name TEXT NOT NULL,
  effects BLOB NOT NULL,
  PRIMARY KEY(account_id, part_instance_id),
  UNIQUE(account_id, inventory_ordinal),
  FOREIGN KEY(account_id) REFERENCES profiles(account_id) ON DELETE CASCADE
);

CREATE TABLE brawler_part_slots(
  account_id BLOB(16) NOT NULL,
  brawler_id BLOB(16) NOT NULL,
  slot_index INTEGER NOT NULL CHECK(slot_index BETWEEN 0 AND 3),
  part_instance_id BLOB(16) NOT NULL,
  PRIMARY KEY(account_id, brawler_id, slot_index),
  UNIQUE(account_id, part_instance_id),
  FOREIGN KEY(account_id, brawler_id)
    REFERENCES brawlers(account_id, brawler_id) ON DELETE CASCADE,
  FOREIGN KEY(account_id, part_instance_id)
    REFERENCES weapon_part_instances(account_id, part_instance_id) ON DELETE RESTRICT
);
```

- Add explicit SQL byte-length/name/effect-payload checks where SQLite can express them. Rust owns
  typed effect decoding and semantic bounds.
- Migration changes structure only. `load_or_create` grants starter set revision 1 in the same
  transaction when `starter_part_set == 0`, inserts exactly eight server-ID instances in authored
  order, then advances the marker. An entropy, insert, validation, or commit failure leaves both
  marker and inventory unchanged, so retry cannot duplicate the grant.
- A new account is created and seeded atomically at initial revision. An existing v1 account is
  seeded once on first v2 load and its profile revision advances once; brawler revisions do not.
- Existing instances retain their exact effect BLOB when a later starter definition changes. A
  missing/invalid definition or effect rejects the unsafe account record and reports the fault; it
  does not seed over, reroll, delete, or reset it.
- Equipment replacement validates optimistic revisions and ownership inside one write transaction,
  deletes/reinserts only the target brawler's occupied slot rows, advances revisions, reloads the
  whole snapshot, and commits. Any failure rolls the transaction back.
- Startup validation, `integrity_check`, `foreign_key_check`, WAL handling, fail-fast behavior,
  corruption preservation, backup refusal-to-overwrite, and tested restore remain the M01
  contracts. Restore tests now compare inventory, slots, and revisions too.

### Authority, ECS ownership, and schedule composition

```text
client equipment draft
  -> ordered reliable EquipWeaponParts intent
  -> lobby ProfileAuthority cache/revision/queue checks
  -> pure catalog-aware part resolution
  -> bounded storage command
  -> exclusive SQLite transaction
  -> accepted whole ProfileSnapshot
  -> client cache replacement

queue admission
  -> lock session against profile mutations
  -> read one accepted brawler + owned instances
  -> canonicalize modifiers and resolve weapon
  -> freeze V3 match snapshot + fingerprint
  -> match worker re-resolves and verifies
```

- `WeaponPartCatalogResource` is shared immutable authored content installed with gameplay content.
  Persistent inventory is not an ECS component and no part entities are spawned.
- The lobby `ProfileAuthority` remains the only accepted in-memory cache. It validates the full
  equipment candidate and resolved weapon before submitting storage work; the storage transaction
  independently enforces revisions, ownership, slot bounds, and uniqueness.
- Reuse the existing ordered lobby phase: storage completions update the accepted cache before new
  profile commands, and profile commands remain serialized before queue admission. Do not add a
  second event bus or schedule.
- The client `ClientProfileModel` remains the accepted mirror. A separate small
  `WeaponEquipmentDraft` resource owns unsaved slot choices, focused slot/item, and preview result
  only while its overlay is open. Closing/canceling discards it; an accepted outcome replaces the
  profile cache; rejection retains the draft and last accepted snapshot.
- Use change-aware render keys or existing dirty checks to rebuild the equipment UI only when the
  accepted snapshot, draft, focus, pending state, or layout changes. Presentation never becomes the
  save or navigation authority.
- Server-only SQLite feature isolation, one profile storage thread, fatal executor failure, and
  match-worker storage isolation remain unchanged.

### Network and immutable match handoff

- Extend the existing `ProfileCommand`, `ProfileOutcome`, `ProfileSnapshot`, and lobby welcome
  payloads. Keep the ordered reliable `ProfileChannel`; no new channel or replicated component is
  needed.
- Advance the one global protocol/content compatibility contract normally because shared message
  shapes and the authored content envelope change. Do not add a V2 profile compatibility decoder or
  a per-part message version.
- Replace `MatchBuildSnapshotV2` with V3 containing permanent fighter/base IDs, mutable ability IDs,
  brawler ID/revision, fixed-order `CanonicalWeaponModifiers`, and the accepted resolved identity.
  It contains no account ID, part instance/definition ID, name, type, icon/model, slot index, rarity,
  or acquisition source.
- Queue admission resolves from the accepted profile after acquiring the queue lock. The V3 snapshot
  is immutable even if the player later leaves queue or edits the brawler; the active match never
  consults the profile.
- Match admission re-resolves the base plus canonical modifiers with its active catalog and fighter,
  checks the recipe fingerprint/identity, and rejects malformed, oversized, incompatible, or
  mismatched snapshots before spawning gameplay state.
- Keep `MAX_MATCH_BUILD_SNAPSHOT_BYTES == 255` and add a maximum-value encoding test. Also retain the
  routing manifest and 1v1/2v2/3v3 maximum-roster size tests.

### Dashboard editor and preview

- Add an “Equipment” action to the selected brawler card/editor. It opens a responsive layout with
  four generic numbered slots, the eight owned part cards, a selected-part effect description, and
  current-versus-candidate weapon summary.
- Part cards show persisted name, presentation type, concise signed effects, and “Equipped by …”
  when unavailable under profile-wide exclusivity. Type never filters or routes a slot.
- Controller/keyboard/pointer actions select a slot, move through inventory, equip, unequip, save,
  and cancel. Empty is a first-class slot choice. Focus survives a responsive layout rebuild and
  remains visible at 1280x720 and the existing compact supported size.
- The preview uses the same pure catalog/part resolver against the local accepted snapshot and
  draft. It presents capacity, damage per delivery, fire interval/rate wording, refill/recharge,
  semantic reach, and Slow. It shows a clear invalid reason and disables Save if resolution fails.
- Save sends the full four-slot candidate and disables mutation/admission controls while pending.
  The UI does not claim success until the accepted snapshot returns. Queue lock disables opening or
  saving equipment locally while the server still rejects forged/racing edits.
- No part mesh, fighter attachment, projectile visual, rarity color, acquisition animation, or shop
  affordance is added.

### Balance Lab and content behavior

- Register the part catalog in the gameplay content fingerprint and in every lobby/match/direct
  composition that resolves a saved brawler.
- Keep starter part definitions fixed/read-only in the M02 Balance Lab UI. Its live base tuning
  transaction must re-resolve representative equipped configurations, including all eight starters
  and the Frost/base-Slow merge, before accepting a base change.
- Active admitted matches retain their V3 modifiers and resolved fingerprint. A later Balance Lab
  base update follows the existing active-match isolation behavior; new admissions resolve against
  the new accepted base catalog.
- Practice bots may use empty slots or deterministic starter combinations, but their snapshots must
  use the same V3 resolver and never synthesize inventory ownership.

## Implementation plan

### 1. Shared part model and resolver

- [x] Add stable part definition/instance IDs, bounded instance/effect models, canonical modifier
  accumulator, and focused errors.
- [x] Add the embedded eight-part starter catalog, validation, canonical material, and global content
  fingerprint integration.
- [x] Implement deterministic flat/percentage/status aggregation, semantic property mapping,
  derived Straight lifetime, final configuration validation, and permutation-independent
  fingerprints.

### 2. Profile model and schema v2

- [x] Extend snapshots/brawlers/commands/outcomes with inventory, four slots, and full equipment
  replacement while preserving creation-field immutability.
- [x] Add schema `1 -> 2`, atomic one-time starter seeding, stable load ordering, relational slot
  integrity/exclusivity, transactional equipment replacement, and semantic load validation.
- [x] Extend corruption, migration, backup, and restore coverage to exact instance effects and
  equipment.

### 3. Authority, protocol, and match handoff

- [x] Extend profile authority/client command lifecycle and queue-lock handling without adding a
  channel or blocking schedule work.
- [x] Install the V3 compact canonical-modifier snapshot, retain the 255-byte bound, and update
  lobby, routing, Practice, admission, direct fixtures, and match resolution.
- [x] Prove match workers receive no account/inventory/storage authority and combat still consumes
  only `ResolvedWeapon`.

### 4. Dashboard and preview

- [x] Add the equipment draft, four-slot/inventory editor, signed effect descriptions, compatible
  preview, pending/error feedback, and save/cancel lifecycle.
- [x] Extend pointer, keyboard, and controller navigation plus responsive wide/compact layouts.
- [x] Ensure queue state disables local edits and forged/racing commands remain server-rejected.

### 5. Balance Lab, automation, and documentation

- [x] Make base-tuning validation and bot/human snapshots part-aware without adding acquisition or a
  part-tuning web editor.
- [x] Keep headless clients on valid empty equipment while focused resolver/profile/handoff tests
  prove non-base weapon behavior without adding another asynchronous bootstrap mutation.
- [x] Reconcile enduring weapon, UX, networking, server architecture, operator, README, and V7
  roadmap documentation with the implemented contract.

## Verification plan

### Pure model, catalog, and resolver tests

- Definition/instance ID and wire/SQLite round trips; zero/malformed ID rejection; bounded names,
  effects, inventory counts, and encoded snapshots.
- Starter catalog ID/key/order uniqueness, fixed eight-instance grant, no-op/duplicate/oversized
  effect rejection, and gameplay fingerprint sensitivity only to gameplay facts.
- Exact flat-then-percentage arithmetic, positive/negative values, clamp boundaries, nearest-round
  ties, reach milliunit quantization, and overflow-safe accumulators.
- Capacity/refill mapping for Magazine and Charges; Reach mapping for Straight/Lobbed/MeleeArc;
  Straight lifetime recomputation; all-hostile-damage modification; unsupported-effect rejection.
- Base Slow plus one/multiple Frost parts produces one clamped `StrongestRefreshes` effect with
  deterministic duration; no applicable bundle and full-bundle cases reject instead of ignore.
- Slot permutation and equivalent distinct instances produce the same canonical modifiers and
  weapon fingerprint. Duplicate use of one instance rejects.
- Exhaustively resolve every combination of zero through four of the eight starter parts against
  every launch base and all three fighter profiles.

### Storage and authority tests

- Fresh schema 0 creates v2; an exact v1 fixture migrates to v2 without changing brawler IDs,
  immutable choices, abilities, selection, ordinals, or brawler revisions.
- Fresh/existing account starter seeding, marker/revision behavior, reconnect idempotence, entropy or
  injected transaction failure rollback, cap enforcement, and no duplicate grants.
- Four empty/occupied slots, full replacement, unequip, same-definition distinct instances,
  duplicate/unowned/missing/already-equipped instance rejection, target deletion retaining
  inventory, and stable load order.
- Stale profile/brawler revisions, duplicate request retry, queue lock, executor backpressure,
  storage failure, and semantic resolution failure preserve the last accepted state.
- Malformed effect BLOB, bad definition, dangling/cross-account slot, foreign-key failure, and
  corruption preserve the database and reject unsafe data without reseeding or reset.
- Backup/restore after WAL writes recovers exact instance IDs, ordinals, names, effect payloads,
  slots, selection, and revisions.

### ECS, network, and process tests

- Ordered profile completion/command/queue admission proves an equipment edit racing queue
  admission has one deterministic outcome and accepted admission freezes the exact brawler revision.
- Client preview equals server resolution for representative and maximum modifier sets; rejected
  outcomes retain the draft and accepted snapshot; reconnect replaces stale UI state.
- Maximum `ProfileSnapshot`, `ProfileOutcome`, lobby welcome, V3 match snapshot, routing manifest,
  and maximum-roster codecs stay within their declared bounds.
- Separate-App and routed process coverage proves the match worker re-resolves the admitted
  fingerprint, executes modified capacity/damage/cadence/reach/Slow, and has no profile inventory or
  database access.
- Balance Lab accepts safe base tuning only when part combinations remain valid, rejects a tuning
  that invalidates equipped resolution, and preserves already admitted matches.
- Client, lobby worker, supervisor/logical-server, and restored-database restart paths recover the
  exact equipment.

### Canonical gates and visual checks

- Run `just check`, `just lint`, `just test`, and routed `just e2e 2`, `just e2e 4`, and
  `just e2e 6` through the production saved-profile path.
- Retain server feature-isolation and sole 3D renderer checks. Prove no client SQLite dependency and
  no match-worker storage authority were introduced.
- Native 1280x720 and compact supported-size checks cover inventory browsing, four empty/occupied
  slots, type-as-label behavior, current/candidate preview, invalid and already-equipped states,
  controller focus, save/pending/rejection, reconnect, and queue lock.
- Gameplay checks use at least Expanded Feed, a cadence part, a reach part, and Frosting Module
  across representative Straight, Lobbed, and Melee bases.

## Playtest handoff plan

After automated and native verification, use a fresh persistent environment:

```bash
BRAWLER_DEV_DATA_DIR=target/v7-m02-playtest just run 2
```

The bounded scenario will ask the player to:

1. Confirm the eight starter parts appear once and all four slots begin empty.
2. Equip four mixed parts, compare the preview with the base, save, restart, and recover the exact
   names/slots/effects.
3. Unequip/swap parts and confirm type labels never restrict a slot.
4. Try to use one physical instance on another brawler and assess the ownership explanation.
5. Queue and confirm equipment is unavailable/rejected, then play modified Straight and Frost
   weapons in Practice/multiplayer.
6. Restore an operator backup and confirm the pre-change inventory/equipment returns.

Requested observations: whether eight parts provide understandable choices, whether the four-slot
interaction is simple, whether tradeoffs and preview wording are trustworthy, whether
profile-wide instance exclusivity is intuitive, and whether the in-match changes match the preview.

## Verification evidence

Automated verification completed on 2026-08-22 from the implementation tree:

- `just check` passed every role, including the Balance Lab web build and server feature graph.
- `just lint` passed formatting, the Balance Lab TypeScript/Vite build, Clippy for routing, client,
  server, and Balance Lab, server feature isolation, and the sole-3D-renderer guard.
- `just test` passed routing/process tests; 419 client tests; 327 server tests; 337 Balance Lab
  tests; the focused Balance Lab network scenario; all 82 authority/replication integration
  scenarios; and all 14 performance gates.
- Resolver coverage exhaustively accepts every zero-to-four combination of the eight starter parts
  on all four launch weapon bases and proves slot permutation preserves modifiers/fingerprint.
- Profile coverage proves schema-v1 migration, one-time starter grant, transactional equip/reload,
  exact backup/restore, corruption refusal, the 128-instance profile bound, and the V3 snapshot's
  retained 255-byte routing bound.
- Routed process E2E passed exact 1v1, 2v2, and 3v3 rosters through `just _e2e-matrix`.
- A fresh native `just run 1` instance launched and shut down cleanly. The Bevy window exposes no
  macOS accessibility application, so automated UI inspection could not validate its rendered
  contents; wide/compact equipment interaction and gameplay feel remain the user-playtest gate.
- After the first user-feedback correction, all 420 client tests and client all-target Clippy
  passed, including the new selected-brawler preview -> management -> editor -> equipment path.

## Risks and controls

- **Modifier ambiguity:** use basis points, semantic property names, one reviewed rounding helper,
  fixed accumulator order, and exhaustive permutation/combination tests.
- **Structural weapon invalidation:** transform only the demonstrated six M02 effect families,
  recompute the one derived Straight field, and finish through the existing validator.
- **Snapshot growth:** send compact canonical modifiers to matches, retain the 255-byte test, and
  measure maximum profile/welcome fixtures before changing their explicit bounds.
- **Silent owned-item changes:** persist exact instance effects, seed with an atomic revision marker,
  and reject unknown/unsafe records without reroll or reset.
- **Authority duplication:** keep one lobby cache and one SQLite owner; client preview is advisory
  and match re-resolution is immutable verification, not another persistence owner.
- **UI complexity:** use four generic numbered slots, one local draft, one full-array save, and the
  existing controller navigation/layout system. No slot types, drag-only interaction, acquisition,
  or part meshes enter M02.
- **Balance spikes:** seed sidegrades, clamp Frost, exhaustively resolve all starter combinations,
  use Balance Lab validation, and request targeted gameplay feedback before closeout.

## Milestone exit criteria

- The four review recommendations are accepted or revised and recorded before implementation.
- One fixed eight-instance starter grant is atomic, idempotent, bounded, persisted exactly, and
  recovered through reconnect, restart, backup, and restore.
- Four generic slots accept empty or four unique owned instances without type rules; profile-wide
  instance exclusivity follows the accepted review decision.
- Typed effects aggregate flat then percentage, clamp/round once, merge Slow once per status kind,
  reject inapplicable effects, and resolve every starter combination through the existing
  `WeaponConfiguration` validator.
- Slot permutation and presentation metadata cannot affect canonical modifiers, resolved behavior,
  or recipe fingerprint.
- Dashboard preview, controller/pointer editing, pending/rejection feedback, persistence, and queue
  lock are usable at wide and compact supported sizes.
- Queue admission freezes a bounded V3 snapshot at or below 255 bytes; match workers verify and
  execute the exact resolved weapon without account, inventory, storage, or acquisition state.
- Schema v1 migration, malformed/corrupt data handling, WAL transactions, operator backup, and
  tested restore preserve the M01 recovery guarantees.
- Focused tests, canonical role/lint/test gates, routed 1v1/2v2/3v3 E2E, native visual/gameplay
  checks, user feedback triage, and learn-from-errors review pass before M02 is marked complete.

## Feedback and learn-from-errors

The first user playtest found that clicking either selected-brawler card appeared to flash the
Dashboard without opening creation, selection, editing, or equipment. This was implemented now:
both prominent brawler targets had been wired directly to `SelectNextBrawler`, which becomes a
no-visible-change server transaction when only one brawler exists. They now open the existing
brawler-management overlay; that surface owns Create, Select Next, Edit, Delete, and the nested
weapon-equipment editor. A focused pointer-path regression covers Dashboard preview -> management
-> editor -> equipment. User validation of the corrected route is pending.

The follow-up playtest found that the equipment list declared clipped overflow without owning a
Bevy `ScrollPosition`, leaving Save below the reachable viewport. Implemented now: slots and
inventory use a bounded mouse/controller-scrollable region, scroll survives draft rebuilds, focused
inventory controls are kept visible, and Save/Cancel remain in a fixed footer. The regression now
also proves wheel movement, an enabled fixed Save action for a valid preview, and successful Save
dispatch. User validation of the corrected layout is pending.

Balance Lab feedback then found every numeric Apply rejected as `unsupported apply schema`. The V7
server and returned snapshot had advanced to schema 2, while the web client still hardcoded schema
1 in Apply and Restore envelopes. Implemented now: Apply derives its envelope schema from the
authoritative draft snapshot and Restore uses the authoritative state schema, removing the duplicate
frontend version constant. The production TypeScript/Vite build passes; user validation of Apply is
pending.

During verification, automatically equipping starter parts in the headless bootstrap added a
second asynchronous SQLite mutation before queue admission and starved the tight deterministic
queue harness. The automation was removed: headless clients keep legal empty slots, while focused
resolver, persistence, snapshot, and admission tests own part behavior. Reusable lessons: prominent
Dashboard affordances must enter the complete management flow instead of performing an ambiguous
single mutation, and test-only bootstrap should not expand product state merely to create evidence;
exercise the owned transaction explicitly at the narrowest verification boundary.
