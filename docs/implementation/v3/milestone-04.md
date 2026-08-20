# V3 Milestone 04 — 2D world-renderer retirement, readability, performance, and closeout

| Field | Value |
|---|---|
| Status | Verifying |
| Depends on | M01–M03 complete |
| Outcome | Retire residual 2D world-renderer assumptions, validate readability and native performance, and close V3 without changing planar authority |

## Research question

What is the smallest finalization slice that proves the supported 3D client is readable, bounded,
performant, lifecycle-safe, and free of obsolete 2D gameplay-world rendering or migration toggles?

## Goal and scope

M04 closes the renderer migration. It does not add gameplay or a second rendering architecture.
The supported client remains an orthographic 3D presentation over replicated/interpolated planar
authority. Screen-space menus and HUD remain Bevy UI.

### Included

- remove residual packaged 2D world art, image handles, obsolete authored renderer names, direct
  world-sprite feature selection, stale tests, and migration wording;
- audit the gameplay-world source and feature graph for legacy branches or planar XY `Transform`;
- tune camera, cover, terrain, lighting, shadows, materials, anti-aliasing, objective readability,
  and reduced-effects behavior without changing authoritative dimensions;
- add bounded opt-in native release render evidence;
- prove map/terrain/restart/reconnect teardown without monotonic presentation-asset growth;
- run the canonical role, network, performance, and supervised visual closeout matrix;
- reconcile V3 feedback, backlog, documentation, and learning.

### Outside M04

- 3D physics, height authority, jumping, walkable walls, or protocol `Vec3` state;
- new maps/themes/content or bulk Kenney import;
- perspective/orbit/cinematic cameras or gameplay zoom;
- custom pipelines, stencil outlines, x-ray silhouettes, decals, deferred rendering, LOD, GPU
  particles, bloom, depth of field, or dynamic time of day;
- general benchmarking infrastructure or hardware claims beyond the recorded reference machine;
- unrelated routing, matchmaking, hosting, account, AI, progression, or balance work.

## Research result

M04 is feasible without changing the server, protocol, gameplay schedules, or physics dependency.
M01–M03 already made 3D the sole gameplay-world renderer and centralized XY-to-XZ conversion. The
remaining work is retirement, tuning, evidence, and closeout.

The inspected reference machine is an Apple M3 with 10 GPU cores, macOS 26.5.1, Metal 4, and a
2880×1864 built-in display. Reports still record the actual adapter and window.

### Baseline audit

| Area | Finding | M04 disposition |
|---|---|---|
| world 2D types | no `Sprite`, `Text2d`, `Mesh2d`, or `ColorMaterial` remains in client combat/world presentation | preserve with a source gate |
| UI camera | `Camera2d` renders screen UI above `Camera3d` | retain; it is not a world renderer |
| sprite feature | UI transitively needs sprite crates; Brawler also selects `bevy/bevy_sprite` directly | remove only the redundant direct feature |
| packaged art | two team PNGs and the facility tileset remain; only team handles affect readiness | delete files, handles, manifest rows, and unused licenses |
| map authoring | unused `VisualPlacementKind::Sprite`; built-in maps use `TiledRectangle` | delete the unused variant/branch without a compatibility layer |
| fallback | `BRAWLER_FORCE_PRIMITIVE_WORLD` selects GLB fallback, not 2D vs 3D | retain and document the distinction |
| lighting | one ambient and one directional light; no explicit AA/shadow-role policy | lock the selected policy |
| reduced effects | combat cues shrink, terrain debris does not | complete transient reduction while retaining state feedback |
| performance | fixed-tick gates exist; no release native frame distribution | add opt-in bounded render evidence |
| lifecycle | terrain updates meshes in place; repeated generated-map asset counts are unproven | add churn/count gates |

## Selected technical specification

### 1. Retirement boundary

Remove:

- `assets/brawler/fighters/{team_blue,team_red}.png` and
  `assets/brawler/maps/facility_tileset.png`;
- their manifest entries, handles/readiness rows, fixtures, and license files when no retained pack
  asset still needs them;
- direct `bevy/bevy_sprite` from `bevy-client`;
- unused `VisualPlacementKind::Sprite` and its resolver branch;
- stale renderer-choice, sprite-fallback, pixel-density, and y-sort prose/tests/code.

Retain Bevy UI, its `Camera2d`, text/UI features, PNG decoding for retained GLB textures, and UI's
transitive sprite crates. Retain `BRAWLER_FORCE_PRIMITIVE_WORLD`; it proves the supported 3D
primitive fallback. The embedded recipes do not use the removed enum variant and resolved network
snapshots do not carry it; fingerprint/network tests stop the work if that assumption is wrong.

### 2. Readability and occlusion

The implementation starts with camera/geometry/material tuning only:

- retain fixed orthographic projection and centralized projected-footprint clamping;
- test the current 55-degree elevation and 900-unit vertical span before changing them;
- keep visible cover footprints and objective boundaries identical to authoritative 2D shapes;
- adjust only presentation height/material when the matrix shows genuine occlusion;
- keep floors calm, teams distinct, neutral cover subordinate, health green, invalid previews red,
  and Hot Zone fill subordinate to its exact boundary;
- retain `Tonemapping::None` and explicitly select four-sample MSAA unless evidence fails;
- retain one directional plus ambient light;
- transparent/unlit previews, objectives, health, status, trails, projectiles, debris, and cue
  effects do not cast shadows; opaque actors/cover/terrain may cast restrained shadows.

No fade or outline is planned: M01–M03 produced no accepted blocker. If tuning still fails, return
to specification review instead of silently adding x-ray rendering, per-wall materials/raycast
fades, or a custom pipeline.

### 3. Reduced-effects contract

`ClientShellSettings::reduced_combat_effects` remains presentation-only:

- retain current shorter/smaller combat cues;
- use a smaller terrain-debris bound, scale, and lifetime; omit redundant burst debris;
- keep objective boundaries, previews, health/team identity, status, projectiles, sentries, and dash
  direction because they communicate gameplay state;
- apply setting changes to new transients immediately and never alter authority.

Focused tests assert bounds and essential-family preservation.

### 4. Opt-in native render evidence

Add `src/client/presentation_3d/diagnostics.rs`, installed only when a windowed client has an
explicit render-report path. Normal, headless, and server runs remain inert.

The CLI/config surface must:

- reject headless use, empty paths, zero durations, and durations above 120 seconds;
- default to 10 seconds warm-up and 30 seconds measurement;
- use `FrameTimeDiagnosticsPlugin` with at most 8,192 measurements;
- consume raw `FRAME_TIME` history after readiness/warm-up, not averaged FPS;
- write once atomically without silent overwrite, then request clean client exit;
- remain separate from gameplay authority and the existing network closeout schema.

The versioned report contains:

- schema/version/commit, debug-vs-release, OS/CPU, adapter/backend;
- window size, render profile, imported/fallback and reduced-effects modes;
- warm-up/measurement durations and sample count;
- frame p50/p95/p99/max and counts above 25/50/100 ms;
- entity, `Mesh3d`, visual-root, transient, terrain-chunk, mesh-asset, and material-asset high-water
  and terminal counts;
- map/mode plus fighter/projectile/sentry/effect/debris high-water counts;
- result and first failed threshold.

Percentiles, bounds, stable serialization, missing/duplicate fields, and no-overwrite receive pure
tests. Verbose GPU pass diagnostics/custom profiling are deferred unless this evidence cannot
answer a failed gate.

### 5. Locked native thresholds

The release gate uses 1280×720 native/vsync on the recorded Apple M3 after assets/map are ready:

- at least 1,200 post-warm-up samples;
- p95 <= 18.5 ms and p99 <= 25 ms;
- no frame above 100 ms and at most 1% above 25 ms;
- no monotonic mesh/material/entity growth through specified churn;
- all existing fixed-tick gates remain green, including combined combat/terrain and
  100-fighter/200-projectile cases.

Measure routed Wipeout with normal effects and Hot Zone with reduced effects, each with two active
clients and representative combat/terrain transients. Record 1680×720 high-refresh as diagnostic,
not a gate. Primitive fallback needs correctness/cleanup, not a duplicate performance matrix. If
vsync invalidates the statistic, record raw evidence and return to specification review rather than
weakening the gate. This is not a universal minimum-spec claim.

### 6. ECS ownership and lifecycle

- `WorldPresentationPlugin` stays the sole gameplay-world composition.
- `MapPresentationMember`, `CombatVisualOwner`, `TerrainChunkVisual`, transient markers, and scoped
  scene ownership remain cleanup keys.
- diagnostics only query presentation; they never modify replicated owners or transforms.
- dynamic map meshes reuse cached unit meshes/transforms or remove handles with their generation,
  whichever smaller correction the count test demonstrates;
- terrain chunks continue updating owned meshes in place and release removed generations;
- imported descendant search remains scoped to `WorldInstanceReady.entity`;
- reconciliation remains idempotent after restart/reconnect/duplicate/orphan cleanup.

### 7. Schedule composition

No gameplay schedule changes are authorized.

```text
Update: renderer-neutral sync -> 3D reconciliation/state/cues/cleanup
PostUpdate: interpolation/writeback -> one pose writer -> camera -> transform propagation
Last (measurement only): completed frame sample -> bounded high-water evidence -> one final report
```

The sampler reads a completed Bevy diagnostic value. Its finalizer cooperates with client shutdown
and does not become a second process-closeout owner.

## Implementation plan

1. Remove dead assets/contracts/features/prose and add source/feature audits.
2. Make AA/shadow roles explicit; tune only from captured comparisons; finish reduced effects.
3. Add repeated map/restart/reconnect entity/asset-count tests and fix demonstrated growth only.
4. Implement the bounded report/parser/CLI and one canonical routed release-evidence command.
5. Run focused and complete automated verification.
6. Record native performance/screenshots and hand off the supervised playtest.
7. Classify feedback/backlog/learning and close V3 only after user acceptance.

## Verification plan

### Focused tests

- readiness no longer waits on deleted PNGs and retains GLB/audio degradation;
- embedded maps/fingerprints/network snapshots remain valid;
- UI `Camera2d` and one world `Camera3d` retain distinct ownership;
- AA/shadow roles and reduced-effect bounds/essential visuals are correct;
- repeated map/generation/restart/reconnect teardown does not grow entities/meshes/materials;
- terrain dirty rebuild updates in place and removed generations release ownership;
- imported/fallback promotion, duplicate roots, and orphans remain idempotent;
- report percentile/schema/validation/no-overwrite/exit behavior is deterministic;
- coordinate and positive/negative gameplay-Y projectile-origin regressions remain green.

### Canonical gates

```bash
just check
just lint
just test
just e2e 2
just e2e 4
just e2e 6
git diff --check
```

Implementation adds one named release render-evidence recipe and documents its artifacts. It uses
the routed product topology; legacy direct UDP is debug/comparison evidence only.

### Targeted audits

- no gameplay-world `Sprite`, `Text2d`, `Mesh2d`, or `ColorMaterial`;
- no legacy renderer choice, sprite fallback, pixel-density, or y-sort path;
- no gameplay entity owns render mesh/scene/material or planar XY presentation `Transform`;
- no duplicated coordinate mapping outside `presentation_3d::coordinates`;
- no direct sprite feature and no presentation capability in the server graph;
- every packaged asset has one manifest entry, provenance, and exercised owner;
- primitive override remains asset fallback only.

### Supervised visual matrix

Use two routed native clients and record screenshots/notes for:

- both modes; imported and primitive fallback; 1280×720, 1024×768, and 1680×720;
- center/corners and actors on both sides of permanent/destructible cover;
- positive/negative gameplay Y, muzzle origin, lob landing;
- all deliveries, dash, sentry, slow, knockback, defeat/respawn, terrain erase;
- every Hot Zone state and exact boundary; normal/reduced effects;
- restart, map replacement, reconnect, disconnect, and re-entry.

Observe actor/target visibility, cover ambiguity, team/health/status identity, preview agreement,
objective clarity, shadow noise, z-fighting, stale visuals, clipping, and local/remote differences.

## Risks

| Risk | Mitigation |
|---|---|
| direct sprite removal breaks UI | distinguish direct feature from UI transitive dependency; compile/UI smoke |
| enum removal changes compatibility | recipes do not use it; fingerprint/catalog/network tests stop on conflict |
| tuning changes collision perception | never change footprint/boundary; height/material only |
| fade/outline scope expands | return to specification review |
| report measures loading | readiness plus warm-up gate |
| vsync distorts percentiles | raw distribution plus high-refresh diagnostic |
| diagnostics affect normal runs | explicit report-path composition only |
| assets grow after churn | repeated ownership/count tests |

## Exit criteria

M04 may move to `User playtest` only when retirement/audits, readability, lifecycle counts,
canonical/e2e/server-isolation gates, and locked native reports pass and a complete handoff is
recorded. M04 and V3 become `Complete` only after feedback classification, affected reverification,
backlog reconciliation, learning review, and user acceptance.

## Implementation and verification evidence — 2026-08-20

Implemented:

- deleted the retired team/facility PNGs, manifest rows, unused licenses, client image handles,
  direct world-sprite feature, and unused `VisualPlacementKind::Sprite` contract;
- selected four-sample MSAA and explicit non-shadow-caster roles for transparent/unlit readability
  geometry while retaining restrained actor, cover, and terrain shadows;
- reduced terrain debris to a 16-entity bound, 0.65 scale, and 220 ms lifetime when reduced effects
  are enabled;
- gave generation-created map meshes explicit ownership and removal after a forty-generation churn
  test demonstrated real monotonic growth in the previous implementation;
- added an opt-in bounded native report with validated CLI durations, raw frame-time percentiles,
  hardware/render context, high-water and terminal counts, atomic no-overwrite output, stable field
  validation, and clean exit;
- added `just v3-render-evidence` and a `just lint` source/feature retirement audit.

Passing evidence:

```text
just check       pass
just lint        pass
just test        pass (routing, 359 client, 302 server, 82 network, 14 performance tests)
just e2e 2/4/6  pass (exact 1v1, 2v2, and 3v3 rosters reached Active)
git diff --check pass
```

The fixed-tick performance suite remained green; representative M04-adjacent results include
3.910 ms combined combat p95, 10.718 ms 24-fighter/24-seam-brush p95, and 1.782 ms
100-fighter/200-projectile p95 on the Apple M3 reference machine.

Native release evidence is currently blocked before sampling. In repeated routed runs, both
optimized clients connected to the lobby and received the same authenticated match grant, but
neither completed its match connection; the empty match worker exited after its bounded admission
window and no report was written. The equivalent debug client completed the handoff and accepted
the authoritative map/terrain snapshot. The locked routed-release requirement is intentionally not
weakened to accept debug or legacy direct-UDP evidence. Diagnose and correct this release-only
handoff before M04 advances to `User playtest`.

## Research sources

### Exact local sources

- `Cargo.toml`, client feature tree, and `scripts/check-server-features.sh`;
- `assets/manifest.ron`, residual asset directories, and `src/client/assets.rs`;
- `src/map/model.rs`, resolver, and `content/v1/maps.ron`;
- `src/client/presentation_3d/{mod.rs,camera.rs,combat.rs,coordinates.rs}`;
- `src/terrain/client/presentation.rs`, `src/config.rs`, `src/client/mod.rs`, and client CLI;
- `src/diagnostics/process.rs` and `tests/performance.rs`;
- cargo-registry Bevy 0.19.1 diagnostics, orthographic, shadow, AA examples and exact diagnostic/
  renderer sources listed during research.

Pinned cargo-registry Bevy 0.19.1 governs exact APIs; checked-in Bevy 0.20-dev is architectural
context only.

### Primary external sources

- [Bevy log diagnostics](https://bevy.org/examples/diagnostics/log-diagnostics/)
- [Bevy 0.19 frame diagnostics API](https://docs.rs/bevy/0.19.1/bevy/diagnostic/struct.FrameTimeDiagnosticsPlugin.html)
- [Bevy orthographic view](https://bevy.org/examples/3d-rendering/orthographic/)
- [Bevy shadow caster/receiver](https://bevy.org/examples/3d-rendering/shadow-caster-receiver/)
- [Bevy anti-aliasing](https://bevy.org/examples/3d-rendering/anti-aliasing/)

## Feedback and closeout placeholders

| Feedback | Decision | Verification |
|---|---|---|
| Imported character faced opposite the targeting line | Implemented during verification by correcting the Kenney model's local `+Z` front to the fighter root's `+X` convention | Focused transform regression test plus supervised visual recheck |
| World health bars appeared grey instead of showing the green fill | Implemented during verification, then superseded by the projected overhead-information cluster below | Focused relation-color tests plus supervised visual recheck |
| Respawned characters retained the completed defeated pose | Initial transition reset was insufficient because Kenney's live clips omit root/leg channels changed by `die`; the imported hierarchy now snapshots its bind pose and restores it before leaving defeated state, treats restored authoritative health as recovery even if marker removal arrives later, then clears the one-shot transition and starts the live loop | Focused bind-pose, recovery-signal, and animation-state regression tests plus supervised respawn recheck |
| Fighter ground circles appeared broken and did not distinguish the local player from allies | Implemented during verification: markers are lifted off the floor to prevent depth fighting, use dedicated unlit/non-shadow-receiving materials, and resolve green/blue/red relative to the controlled fighter | Focused relation and floor-separation regression tests plus supervised visual recheck |
| Fighter facing used a cuboid protruding from the character | Implemented during verification: fighter facing is now a small flat arrowhead integrated into the team ring, with a segmented concave rear edge matching the ring circumference; sentries retain their separate body-direction primitive | Mesh geometry regression test plus supervised visual recheck |
| Fighter overhead information needed relation-colored names, rounded relation-aware health, a white overlapping health amount, and local-only segmented ammunition | Implemented during verification as camera-projected Bevy UI attached to each fighter: local/ally/enemy names are green/blue/red, local and ally health is green while enemy health is red, rounded clipping preserves readable fill, and only the controlled fighter receives one live segment per authoritative weapon-capacity shot. The first playtest exposed overlapping mutable `Visibility` query access between ammunition rows and weapon previews; explicit disjoint filters now encode that ownership. Follow-up tightened the layout with structurally centered text, 20% narrower bars and name text, and a shorter non-local projection box that reserves no ammunition-row gap. | Focused color/ammunition/layout tests, runtime schedule-initialization regression, client compilation/lint, and supervised visual recheck |
| Awaiting supervised playtest after verification | — | — |

### Learn-from-errors review

Complete after feedback. Revisit UI/world dependency distinction, evidence validity, asset churn,
occlusion scope control, and whether a recurring lesson merits a repository/Codex skill change.
