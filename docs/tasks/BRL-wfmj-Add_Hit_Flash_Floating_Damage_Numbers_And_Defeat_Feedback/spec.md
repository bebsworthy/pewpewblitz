# Outcome

Players see and hear every hit they deal and take, which closes the largest feel gap with the genre.

# Context

A search of `src/` finds no damage numbers, hit flash, hit-stop, recoil, squash, or kill feed. Combat already replicates outcome cues, and presentation already has bounded `VfxRequest`/`AudioRequest` boundaries and a reduced-effects path.

# Scope

- **Hit flash:** about 80 ms emissive/white pulse on damaged fighters, deployables, and damageable objects, compatible with the concealment material variants (`presentation_3d/combat/fighter_feedback.rs`).
- **Floating numbers:** world-projected, pooled and bounded. Pop to about 1.3x, rise, and fade. Distinct styling for damage dealt, damage taken, and healing. Merge or stack rule for rapid multi-hit weapons (spray, fields).
- **Defeat feedback:** a pop/puff VFX plus a sound, and a brief marker at the defeat point.
- Everything is driven from existing replicated outcomes and cues. No new authority.
- A reduced-effects variant, and authored tuning in the VFX/audio catalogs.

# Out of scope

Hit-stop that changes simulation time, kill feed (HUD redesign ticket), and new combat mechanics.

# Acceptance criteria

- Every damaging hit visible to the observer produces a flash and a number within one frame of the cue arriving.
- Concealed enemies never leak a flash or number to observers who are not allowed to see them.
- In 3v3 with spray/field weapons, numbers stay legible (merge rule holds) and the pool never exceeds its bound.
- Defeats always show the defeat effect and play the sound.
- There are no gameplay changes, and network tests pass.

# Verification

- Focused tests for pooling, merging, and concealment gating.
- `just test`, `just practice-e2e wipeout-3v3`.
- A native playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Design decisions (2026-11-12, before implementation)

Research findings:

- The server already filters every `CombatCue` per connection through `ObserverVisibilityCache`
  (`combat::authority::send_combat_cues` / `combat_cue_subjects`): a damage, healing, or defeat
  cue whose source or target fighter is concealed from an observer never reaches that client.
  Concealed enemies are also not replicated to that observer.
- Defeat already has a pop VFX (`defeat` recipe under `combat.damage` + AttachedStatus +
  Interruption) and a sound (`defeat` audio cue). The sound occurrence is the attack id, so two
  defeats from one attack (or reused DoT attack ids) collapse into one sound.
- Damageable objects report damage via `WorldObjectCue::Damaged` (barrels) and
  `ReceivedHeistObjectiveCue` (Heist idols); both carry an authoritative amount.
- Healing arrives as `CombatCue::EffectApplied { effect: Healing { amount (applied) } }`.

Decisions:

1. **One client presentation module** `src/client/presentation_3d/hit_feedback.rs` (+
   `hit_feedback/{flash,numbers,tests}.rs`). It only reads `DeduplicatedCombatCue`,
   `ReceivedWorldObjectCues`, and `ReceivedHeistObjectiveCue`; it never computes damage and writes
   only presentation state. No protocol, server, or gameplay change.
2. **Concealment gating:** server filtering is the primary gate. The client adds a fail-closed
   check: a fighter/deployable/NPC target must resolve to an entity replicated to this observer
   (`NetworkEntityId` present), otherwise no flash and no number. World objects/idols must match a
   live presentation identity. Focused tests cover an unreplicated (concealed) target.
3. **Hit flash:** a `HitFlash` component on the presentation root (fighter, sentry, zombie,
   barrel, Heist idol). One `apply_hit_flash_materials` system runs in `Animate` after every
   material writer (concealment, barrel damage tint, Heist status) and swaps each descendant
   `StandardMaterial` to a cached white/emissive flash variant derived from the *current* material
   (so a concealed translucent variant flashes translucent), restoring the base when the flash
   ends. The concealment system resolves a flash variant back to its base before binding or
   comparing, so the two writers never fight. Variant cache is bounded by a code ceiling. Ground
   team rings/facing markers are excluded. Duration about 80 ms; reduced effects uses a dimmer,
   shorter authored flash.
4. **Floating numbers:** a fixed pool of screen-space UI text slots (Lilita One via `hud_text`,
   above overhead plates in z-order), spawned once and reused; the bookkeeping (`FloatingNumberBook`)
   is pure and tested. Capacity is authored and clamped to a code ceiling (32); when full the oldest
   entry is evicted, so the bound can never be exceeded. Styles: damage dealt by the local player
   (incl. its deployables/ultimates/environment it initiated), damage taken by the local player,
   healing (`+N`), and observed third-party damage (smaller, muted). Numbers are world-anchored at
   the target's presented position at spawn, projected each frame after camera update, offset to the
   side of the body so they do not sit on the overhead plate stack.
   **Merge rule:** a new hit on the same subject with the same style merges into a live number if
   the last merge was within `merge_window_millis` and the number is younger than
   `max_merge_millis`; the merged number sums the amount, re-anchors to the target, and re-pops.
   Otherwise a new number starts with a deterministic lateral offset per live number on that
   subject. Pop about 1.3x, rise, fade. Reduced motion: no pop and no rise (fade only).
5. **Defeat feedback:** keep the existing VFX path; add a smoke-puff particle layer and a brief
   ground defeat marker layer to the authored `defeat` recipe (and a marker layer to
   `defeat_reduced`). Defeat audio occurrence switches from attack id to the unique cue event id so
   every defeat plays the sound. Camera shake stays with the BRL-e4xj rig (no second shake path).
6. **Tuning:** flash and number tuning live in a new client-only catalog
   `assets/catalogs/hit_feedback.ron` (schema-validated, code-owned ceilings, like
   `camera_rig.ron`); defeat puff/marker tuning lives in `assets/catalogs/vfx.ron`. Neither enters
   the gameplay fingerprint or the server feature graph.

Verification note: `just practice-e2e wipeout-3v3` from the original spec plus the canonical
`just lint`, `just test`, `just e2e 2`.


# Implementation (2026-11-12)

Files:

- New `src/client/presentation_3d/hit_feedback.rs` (catalog, cue mapping, ingest system,
  `install`), `hit_feedback/flash.rs` (HitFlash + bounded variant cache + material pass),
  `hit_feedback/numbers.rs` (pure `FloatingNumberBook` + pooled UI slots + projection),
  `hit_feedback/tests.rs`; new client-only catalog `assets/catalogs/hit_feedback.ron`.
- `assets/catalogs/vfx.ron`: `defeat` recipe gains `defeat_marker` (ground spark, 1.4 s) and
  `defeat_marker_ring` layers plus a `defeat_puff` smoke particle layer; `defeat_reduced` gains
  `defeat_marker_reduced`. `src/client/vfx/catalog.rs` layer-count test updated to the new contract.
- `src/client/audio/producers/combat.rs`: defeat sound occurrence is the cue event id (every defeat
  plays, including multi-defeats from one attack).
- `src/client/presentation_3d/combat/fighter_feedback.rs`: concealment material writer resolves a
  flash variant to its base before comparing/binding, so concealment and flash never fight.
- `src/combat/cues.rs`: shared `DamageSource::initiating_fighter()`; `combat/authority.rs` uses it
  instead of its private duplicate (no behavior change).
- Small visibility changes: re-export `FighterGroundMarker3d`/`SentryVisual3d` inside
  `presentation_3d::combat`, `ZombieVisual.owner` and `FighterGroundMarker3d.owner` crate-private
  to presentation_3d.
- `docs/11-art-and-presentation-direction.md`: durable hit-feedback contract paragraph.

No protocol, server, gameplay, or fingerprint change. Hit-feedback systems are client-only and run
in `WorldPresentationSet::ConsumeCues` (ingest, before the VFX producer drains the world-object
inbox), `Animate` (flash, after all material writers), and `PostUpdate` (number projection after
camera/transform propagation, before UI prepare).

# Verification (2026-11-12)

ripgrep was not on PATH as a binary; a scratch `rg` wrapper was put first on PATH so the lint
scripts ran for real (`_server-features`, `_v3-world-presentation`, `_map-cleanup` all printed
their pass lines).

- Focused: `cargo test --no-default-features --features client --lib hit_feedback` → 11 passed:
  catalog validity and fail-closed ceilings; style mapping (dealt/taken/observed/healing, zero
  ignored, defeat not a number); unreplicated (concealed) target leaks nothing (pure and ECS);
  merge sums/re-anchors/re-pops, distinct styles stack laterally, window lapse starts a new number,
  expiry frees slots; sustained 10 Hz spray cannot merge forever; 3v3 load (6 subjects × 4 styles
  every 50 ms for 5 s plus bursts) peak live ≤ pool capacity and ≤ 2 numbers per subject/style,
  overflow recycles the oldest, capacity clamps to 32; pop/rise/fade curve and reduced motion; flash
  variant keeps concealed translucency/blend; ECS flash appears on the cue frame, team rings never
  flash, restores base after ~80 ms; ECS flash + concealment writer coexist without churn.
- `just lint` → pass.
- `just test` → pass (client lib 790, server 579/529, network 107, performance, routing, balance-lab).
- `just e2e 2` → pass (exact 1v1 roster reached Active).
- `just practice-e2e wipeout-3v3` → pass (Practice wipeout-3v3 reached Active with one human).
- Native: `BRAWLER_RENDER_COMBAT=1 just v3-render-evidence target/brl0144-render.txt` → passed with
  3 paired captures, no panics, but the scripted combat produced `hostile_hits=0`, so the captures
  do not show a flash or number. Visual confirmation of flash/numbers/defeat marker therefore
  remains for the native playtest below.

# Playtest request

Run `just run 2` → Practice → Wipeout 3v3 (bots), then Heist 1v1 (idol + barrels).

Observe:

1. Hitting an enemy: target flashes white briefly; yellow number pops (~1.3x), rises, fades beside
   the body, not on top of the overhead plate.
2. Taking damage: red numbers on your fighter; camera shake unchanged from BRL-e4xj.
3. Spray/field weapons on one target: one climbing merged number, new number after ~1.1 s; numbers
   stay legible in a crowded 3v3 fight.
4. Restoration healing: green `+N`.
5. Barrels and Heist idol: flash + number when shot.
6. Defeat: pop, small smoke puff, red ground marker for ~1.4 s at the defeat point, defeat sound
   every time (including two defeats from one blast).
7. Concealment: enemies in bushes/cloak never show a flash or number when hit by allies; a cloaked
   ally flashes translucent.
8. Settings: reduced motion → numbers fade in place without pop/rise; reduced combat effects →
   dimmer/shorter flash, smaller defeat effect without puff.

Known limitations: no dedicated hit-confirm/damage-taken sound and no numbers for pickup/regeneration
recovery that arrive without a healing cue (deferred to BRL-zy9d); third-party (observed) numbers
are shown in a muted smaller style — say if they clutter 3v3.

# Learn from errors

- An ECS test first compared ring materials against one fighter's base handle while two fighters had
  distinct bases; assert the invariant (no flash variant on rings) instead of a specific handle.
- A VFX catalog contract test pinned exact defeat layer counts; changing authored recipes needs that
  test updated in the same change — search tests for recipe ids before editing catalogs.
- Lint scripts depend on a real `rg` binary; in this environment `rg` was only a shell function, so a
  PATH wrapper was required for the checks to be meaningful.
- The canned combat render evidence produced no hits, so it cannot substitute for a feel playtest.

# Deferred

- BRL-zy9d (todo, related, parent BRL-q5c4): hit-confirm/damage-taken audio and healing numbers for
  non-cue recovery.

# Playtest feedback (2026-11-13)
- User ran native sessions (`just run 2`, `just run 1`) with several 3v3 Practice matches and accepted the result ("seems fine"). No corrections requested; the open tuning choices in this spec are accepted as implemented. No induced-latency run was reported separately.
- Observed in the session logs, not user-visible: recurring `bevy_render::slab_allocator: Use-after-free` errors at client start and on match exit; tracked separately (see BRL-q5c4 children). Disposition: accepted; no open feedback items.
