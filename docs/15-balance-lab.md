# Balance Lab operator and maintenance guide

## Purpose

Balance Lab is a development-only, server-authoritative tuning console for local Practice matches.
It edits a complete fighter-profile, weapon-base, and supported-ultimate tuning snapshot, validates
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
2. The launcher immediately opens <http://127.0.0.1:5123> in the default browser. The initial page
   may report that the endpoint is unavailable because only Practice workers host Balance Lab.
3. Use the launched client to connect locally and enter Practice. The launcher opens the URL again
   when the worker's Balance Lab endpoint becomes ready.
4. Review **Players & loadouts** for the authoritative human and bot roster admitted to this
   Practice worker. Each card identifies the team, fighter profile, weapon base, ultimate, two
   passives, and effective weapon modifiers. Collapse the panel when more tuning space is useful.
5. Choose a gameplay section and one fighter, weapon, ultimate, world object, or mode. Edit the
   focused draft using the displayed gameplay units and authoritative bounds.
6. Review the changed marker plus applied/default comparison. Values that differ from the
   canonical server defaults are highlighted in red independently of whether they are newly edited
   or already applied. Use **Copy differences** to copy a readable list of every current draft value
   that differs from those defaults, including its field path, default value, current value, and
   gameplay unit.
7. Choose **Apply & reset match** when the draft is ready.
8. Use a field's **Reset** action to restore its applied value, **Revert draft** to discard all
   unapplied edits, or **Restore canonical defaults** to remove the persisted override and reset to
   canonical content.

Accepted tuning is stored in `target/balance-lab/session-v2.json`. The page reconnects to later
Practice workers at the same loopback URL, and each worker validates the persisted snapshot before
installing it. Deleting build artifacts or using **Restore canonical defaults** removes the override.

The current snapshot schema is version 9, the persistence envelope is version 4, and the
non-persisted editor-manifest schema is version 2. The server manifest explicitly identifies every
editable numeric path, gameplay unit, storage conversion, authoritative bound, step, and preferred
control. The browser does not infer editability or limits from serialized field names. It exposes
the three permanent fighter profiles, four canonical weapon-base recipes, the bounded parameters of
all five supported ultimates, oil-barrel health/explosion tuning, Heist safe health, and treasure-
chest/restoration-pickup health, restoration, radius, and lifetime. Structural IDs, terminal
topology, replacement assets, and pickup visual identity remain locked. Persistence envelope 3 is
migrated by filling canonical chest defaults before validation. Snapshot 8 is migrated inside the
current envelope by filling the new fighter-recovery fields from canonical content while retaining
existing tuning. The retired Custom Pulse axes,
named build presets, point budget, and frame passives are not Balance Lab surfaces. Apply validation
re-resolves the complete 3×4 fighter-profile/weapon-base matrix, validates the rebuilt map catalog
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
remains available beyond the editor's ordinary playtest range.

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

Primary implementation ownership currently lives in `src/server/balance_lab/`, with authored build
tuning in `src/builds/definitions.rs`, weapon definitions and map-destruction bounds in
`src/combat/definitions/`, object/chest definitions in `src/map/catalog.rs`, Heist safe rules in
`src/matchplay/heist.rs`, and the operator application in `tools/balance-lab-web/`.

## Scope and limitations

The service is compiled only by the `balance-lab` feature, binds to loopback, and is enabled only
for the canonical local Practice formation. One Practice worker owns the endpoint at a time. Drafts
do not mutate simulation until explicit apply. Remote access, authentication, canonical-content
export, hot/live apply, charts, balancing advice, new ability definitions, passives, and broad
match-rule tuning beyond Heist safe health are outside the current tool. Apply remains an explicit
clean-epoch transaction. A persisted Heist safe value is installed into a new Heist worker, remains
unchanged during unrelated Wipeout/Hot Zone edits, and can be cleared from every Practice mode
through **Restore canonical defaults**.
