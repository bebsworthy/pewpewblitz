# Balance Lab operator and maintenance guide

## Purpose

Balance Lab is a development-only, server-authoritative tuning console for local Practice matches.
It edits a complete global-rule, fighter-profile, weapon-base, and supported-ultimate tuning snapshot, validates
it against concrete engine and wire safety invariants, persists accepted tuning locally, and
applies it by starting a clean authoritative match epoch. It does not modify canonical authored
content.

Use [Weapons and abilities](./03-weapons-and-abilities.md) and
[Fighter and build specification](./02-fighter-model.md) for the gameplay property model. The
[V6 roadmap](./implementation/v6/roadmap.md) and
[milestone](./implementation/v6/milestone-01.md) retain the original implementation history and
evidence. The completed [V10 roadmap](./implementation/v10/roadmap.md) and
[M03 closeout](./implementation/v10/milestone-03.md) record the barrel, Heist-objective, chest, and
restoration-pickup evolution.

## Operator workflow

1. Run `just balance-lab`.
2. Use the launched client to connect locally and enter Practice. Only Practice workers host
   Balance Lab, so the launcher waits for that worker's endpoint and then opens
   <http://127.0.0.1:5123> once in the default browser.
3. Review **Players & loadouts** for the authoritative human and bot roster admitted to this
   Practice worker. Each card identifies the team, fighter profile, weapon base, ultimate, two
   passives, and effective weapon modifiers. Collapse the panel when more tuning space is useful.
4. Choose a gameplay section and one global rule family, fighter, weapon, ultimate, world object,
   or mode. Edit the focused draft using the displayed gameplay units and authoritative bounds.
5. Review the changed marker plus applied/default comparison. Values that differ from the
   canonical server defaults are highlighted in red independently of whether they are newly edited
   or already applied. Use **Copy differences** to copy a readable list of every current draft value
   that differs from those defaults, including its field path, default value, current value, and
   gameplay unit.
6. Choose **Apply & reset match** when the draft is ready.
7. Use a field's **Reset** action to restore its applied value, **Revert draft** to discard all
   unapplied edits, or **Restore canonical defaults** to remove the persisted override and reset to
   canonical content.

Accepted tuning is stored in `target/balance-lab/session-v2.json`. The page reconnects to later
Practice workers at the same loopback URL, and each worker validates the persisted snapshot before
installing it. Deleting build artifacts or using **Restore canonical defaults** removes the override.
After the immutable admission snapshot is validated against canonical content, the Practice worker
re-resolves every admitted human and bot against the persisted Lab catalogs. This deliberately
recomputes runtime recipe identities for equipped weapon modifiers whose base recipe was tuned. If
persisted tuning makes any admitted loadout genuinely invalid, that tuning is ignored for the new
worker rather than crashing or stranding the client during Match Loading.

The current snapshot schema is version 16, the persistence envelope is version 11, and the
non-persisted editor-manifest schema is version 7. The server manifest explicitly identifies every
editable numeric path, gameplay unit, storage conversion, authoritative bound, step, and preferred
control. The browser does not infer editability or limits from serialized field names. Its
**Global** tab currently contains a **Cold & Freeze** section for buildup decay delay/rate, Freeze
duration, and post-thaw immunity. It also exposes the three permanent fighter profiles, seven
canonical weapon-base recipes, and the bounded parameters of all nine tunable ultimates,
including every Sticky Blomb delivery/fuse value, every Splash numeric delivery value, and every
Big Blob parent/secondary value. Oil-barrel health/explosion tuning, Heist safe health, and
treasure-chest/restoration-pickup health,
restoration, radius, and lifetime are also editable. Structural IDs, terminal
topology, replacement assets, and pickup visual identity remain locked. Older supported envelopes
migrate sequentially by filling canonical chest, fighter-recovery, demolition,
elemental-resistance, elemental-field, Cold-capacity, and global condition-rule defaults before validation while retaining existing
tuning. The removed full-build workflow is not a Balance Lab surface. Apply validation
re-resolves the complete 3×7 fighter-profile/weapon-base matrix, validates the rebuilt map catalog
and advertised brawler catalog, and then starts a clean Practice epoch.

The roster view is also non-persisted and read-only. It is projected from the worker's authoritative
admission manifest rather than inferred from client state. The routed match snapshot contains each
selected fighter profile, weapon base, ultimate, passive pair, and the canonical aggregate of all
equipped weapon parts. It does not retain the individual weapon-part identities, so Balance Lab
truthfully presents their effective capacity, damage, timing, reach, and slow modifiers instead of
inventing part names.

Fighter tuning also exposes health recovery per second and the accepted-attack idle delay. Weapon
refill/recharge timing is labeled as recovery for one round or charge. These durations require a
positive authoritative tick value but have no invented balance ceiling; the exact numeric input
remains available beyond the editor's ordinary playtest range. Type timing values as ordinary
decimal seconds. Balance Lab saves the nearest 1/60-second server tick and displays tick-backed
seconds to two decimal places: `0.17 s`, for example, resolves to 10 ticks. Operators do not need to
calculate or enter an exact six-decimal tick multiple.

Each fighter profile also exposes its own Cold capacity and Cold, Poison, and Fire resistance
baseline. Resistance is displayed as a percentage while stored in basis points. The roster always
shows these baselines, including zero values, and distinguishes an equipped passive's additive
bonus from the effective resolved resistance. Cryogenic weapon and field values are labeled as Cold
per hit or per pulse, while the target card supplies the capacity denominator that determines
Freeze timing.

Global Cold lifecycle controls are deliberately separate from fighter profiles. Capacity and
resistance describe how difficult a target is to freeze; the shared decay delay, decay rate, Freeze
duration, and post-thaw immunity describe how the condition behaves for everyone. Timing is shown in
seconds and stored as authoritative ticks. Decay is shown in Cold per second and stored as an integer
Cold-per-tick rate, so the canonical `600 cold/s` value remains the exact `10 cold/tick` fixed-step
rule. Apply and restore install the complete validated rule resource only at the clean match-restart
boundary.

For a straight weapon, **Projectile radius** is the radius of its authoritative circular
`ProjectileBody`, not an explosion radius or decorative effect size. Applying a new value changes
the server collider and sweep, the visible projectile diameter (`2 × radius`), and the local aim
corridor width together. Muzzle-offset validation still prevents the configured body from starting
inside its source fighter. Future non-circular bodies require an explicit shape capability rather
than overloading this radius field.

The Spray base exposes propagation speed, reach, cone angle, linger duration, pulse interval, and
maximum targets as bounded numeric fields. Its geometry-occlusion switch remains structural rather
than an ordinary balance control. Applying any Spray edit starts a clean Practice epoch so no live
spray mixes old and new timing or geometry.

The Spray damage payload also exposes three distance-falloff controls:

- **Falloff start:** distance from the fixed spray source at which damage begins decreasing; pulses
  deal full damage before this point.
- **Falloff end:** distance at which the decrease reaches its minimum; damage decreases linearly
  between the start and end.
- **Minimum damage scale:** damage multiplier used at and beyond the falloff end. `0.5` means 50%
  damage. It affects damage only, not propagation reach or cone occupancy.

The Splash base exposes maximum placement distance, flight duration, visual arc height, landing
clearance, muzzle offset, duration, pulse interval, maximum targets per pulse, and maximum active
areas per owner. Its selected Circle radius or oriented Rectangle half-extents are numeric fields;
the shape variant, map-occlusion rule, two-effect topology, and recipient policies remain structural.
Applying an edit starts a clean Practice epoch so in-flight deliveries and active areas cannot mix
old and new geometry, timing, payload, or capacity rules.

The canonical default fighter starts with `1,000` maximum health, `70` world units/second movement,
`100 health/second` recovery, and `3.0 seconds` of accepted-attack idle delay. The lightweight and
reinforced profiles remain independently authored rather than inheriting these values. Canonical
weapon recovery per round or charge is `1.0 seconds` for Pulse Sidearm, `1.2 seconds` for Scatter
Cannon, `1.6 seconds` for Arc Launcher, `1.0 seconds` for Impact Blade, and `1.5 seconds` for Sticky
Blomb, Spray, and Splash. **Apply & reset match** re-resolves every admitted human and bot, starts a
clean match epoch, initializes health-recovery
inactivity at that epoch, restores starting ammunition, and clears old fire/recovery deadlines.
Draft edits never mutate the running epoch before that explicit apply action.

Ultimate tuning includes Demolition Strike maximum range and destruction radius. Its radius remains
quantized to whole-cell-safe 4-world-unit steps and capped at 64 world units. Weapon world-effect
fields remain available for generic recipe validation, while the built-in Arc Launcher exposes no
terrain-destruction effect.

## Validation principle

The operator is allowed to change balance rules. Balance Lab must not reject a value merely because
it falls outside normal shipping balance policy. A retained constraint must protect at least one
named invariant:

- finite and representable numeric state;
- bounded memory, work, collection size, or serialized payload size;
- deterministic geometry or quantization required by authoritative simulation;
- server/client replication and convergence;
- immutable recipe topology or an implemented runtime capability;
- arithmetic and lifecycle safety such as non-overflowing deadlines.

The UI exposes these bounds before submission, converts ticks to seconds and ultimate milliunits to
world units, and associates a path-qualified server rejection with its field. The server still
revalidates the complete stored snapshot. Authored policy may remain narrower for ordinary content
while Balance Lab uses a wider proven-safe engine ceiling. Expanding a real engine ceiling requires
updating the affected runtime, wire, client-convergence, and capacity tests together.

For fighter profiles, maximum health accepts the complete nonzero `u16` representation
(`1..=65,535`). Movement speed must be finite and greater than zero; the ordinary UI starts at `1`
world unit per second because its control advances in whole-unit steps. These are representation and
runtime invariants rather than shipping balance policy. The existing `1,200` movement ceiling and
world-object health bounds remain separately owned constraints pending their own review.

## Required maintenance contract

Every change to fighter, brawler-build, weapon-property, recipe, validation, resolution, or runtime
state must review Balance Lab in the same change. The author must either update the lab or record why
the property is intentionally unavailable. A gameplay property is not complete when canonical
content can author it but the tuning workflow silently omits, mislabels, rejects, fails to apply, or
fails to reset it.

Review this checklist:

- **Snapshot:** add, remove, or migrate the versioned snapshot field and persistence envelope.
- **Catalog conversion:** keep snapshot extraction and validated working-catalog reconstruction in
  sync with canonical authored content.
- **UI:** expose new numeric leaves, meaningful units, exact bounds, and relevant derived facts.
- **Validation:** distinguish shipping policy from a concrete engine/wire invariant and keep UI
  constraints consistent with server validation.
- **Application:** re-resolve every admitted human and bot and initialize all affected runtime
  components coherently.
- **Reset:** clear or restore every transient state introduced by the property or capability.
- **Identity and networking:** recompute recipe fingerprints and verify revised resolved state
  replicates while authority remains server-owned.
- **Bounds:** update HTTP/persistence/wire-size ceilings and terrain or combat capacity assumptions
  when the expanded representation requires it.
- **Verification:** add focused snapshot/validation tests, fixed-tick apply/reset coverage, and
  network coverage proportional to the changed property.
- **Documentation:** update this guide plus the owning fighter/weapon specification and active
  implementation milestone.

Primary implementation ownership currently lives in `src/server/balance_lab/`, with authored global
condition tuning in `content/catalogs/combat_conditions.ron`, authored build
tuning in `src/builds/definitions.rs`, weapon definitions and map-destruction bounds in
`src/combat/definitions/`, object/chest definitions in `src/map/catalog.rs`, Heist safe rules in
`src/matchplay/heist.rs`, and the operator application in `tools/balance-lab-web/`.

Elemental field timing, range, radius, and effect strength are part of the editable ultimate
snapshot. The snapshot/persistence schema is migrated when these fields, fighter elemental
resistances, Cold capacity, or global condition rules enter the catalog so an older saved lab state receives canonical
values rather than silently omitting the new mechanics.

## Scope and limitations

The service is compiled only by the `balance-lab` feature, binds to loopback, and is enabled only
for the canonical local Practice formation. One Practice worker owns the endpoint at a time. Drafts
do not mutate simulation until explicit apply. Remote access, authentication, canonical-content
export, hot/live apply, charts, balancing advice, new ability definitions, passives, and broad
match-rule tuning beyond the explicit Global-tab sections and Heist safe health are outside the current tool. Apply remains an explicit
clean-epoch transaction. A persisted Heist safe value is installed into a new Heist worker, remains
unchanged during unrelated Wipeout/Hot Zone edits, and can be cleared from every Practice mode
through **Restore canonical defaults**.
