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
[milestone](./implementation/v6/milestone-01.md) retain implementation history and evidence.

## Operator workflow

1. Run `just balance-lab`.
2. Run `just client`, connect locally, and enter Practice.
3. Open <http://127.0.0.1:5123>.
4. Edit the draft, then choose **Apply & reset**.
5. Use **Revert draft** to discard unapplied edits or **Restore defaults** to remove the persisted
   override and reset to canonical content.

Accepted tuning is stored in `target/balance-lab/session-v2.json`. The page reconnects to later
Practice workers at the same loopback URL, and each worker validates the persisted snapshot before
installing it. Deleting build artifacts or using **Restore defaults** removes the override.

The current snapshot schema is version 8 and the persistence envelope is version 4. It exposes the
three permanent fighter profiles, four canonical weapon-base recipes, the bounded parameters of
all five supported ultimates, oil-barrel health/explosion tuning, Heist safe health, and treasure-
chest/restoration-pickup health, restoration, radius, and lifetime. Structural IDs, terminal
topology, replacement assets, and pickup visual identity remain locked. Persistence envelope 3 is
migrated by filling canonical chest defaults before validation. The retired Custom Pulse axes,
named build presets, point budget, and frame passives are not Balance Lab surfaces. Apply validation
re-resolves the complete 3×4 fighter-profile/weapon-base matrix, validates the rebuilt map catalog
and advertised brawler catalog, and then starts a clean Practice epoch.

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

The UI should expose such a bound before submission and the server error should name the concrete
constraint. Authored policy may remain narrower for ordinary content while Balance Lab uses a wider
proven-safe engine ceiling. Expanding a real engine ceiling requires updating the affected runtime,
wire, client-convergence, and capacity tests together.

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
`src/combat/definitions/`, and the operator application in `tools/balance-lab-web/`.

## Scope and limitations

The service is compiled only by the `balance-lab` feature, binds to loopback, and is enabled only
for the canonical local Practice formation. One Practice worker owns the endpoint at a time. Drafts
do not mutate simulation until explicit apply. Remote access, authentication, canonical-content
export, hot apply, charts, abilities, passives, and match-rule tuning are outside the current tool.
