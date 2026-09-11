# Scope

## Product decision

Relationship presentation is part of a VFX recipe's composed visual language. It is not a generic high-elevation overlay rendered independently over every effect.

The exact authoritative gameplay boundary remains mandatory and visually stable. Ownership treatment supplements that boundary without changing gameplay geometry, timing, authority, targeting, collision, or replicated outcomes.

## Required work

- Define a bounded relationship-treatment contract for self, ally, enemy, and neutral presentation that VFX recipes can consume.
- Integrate the treatment into existing effect-owned layers such as the exact boundary, secondary rim, edge rhythm, directional accents, or restrained decorative details.
- Prove the treatment first with the production Fire and Cold persistent-area recipes, then apply the same composable policy to other effect families where relationship is relevant.
- Preserve elemental/style identity: relationship color must not replace or substantially mute the effect's primary palette, motion, or silhouette.
- Avoid color-only dependence. Use at least one additional stable cue where dense combat or common color-vision ambiguity makes hue insufficient.
- Ensure self, ally, enemy, and neutral variants are available through the production signature/resolver path and through VfxLab context fixtures.
- Remove or retire the current generic relationship inset only after the integrated treatment has equivalent readability in production.
- Preserve current client-only ownership. Do not move renderer data into gameplay catalogs or make presentation authoritative.
- Preserve bounded allocation, deterministic request ordering and variation, late-join reconstruction, cleanup, and primitive fallback behavior.

## Presentation constraints

- The exact gameplay boundary must remain distinguishable from decorative flames, shards, mist, particles, and floor coloration.
- Ownership treatment must not resemble another damaging zone, additional radius, selection ring, or collision boundary.
- It must remain readable over representative light, dark, warm, and cool map surfaces.
- Normal, reduced-effects, and primitive modes must retain relationship information.
- Overlapping self, ally, and enemy areas in representative 3v3 density must remain interpretable without relying on bloom.
- The implementation should reuse recipe layers and material parameters. Do not create one bespoke shader or built-in-ID branch per weapon.

## Acceptance criteria

1. A locally owned Fire area no longer renders the current dominant green annulus above the flames.
2. Fire and Cold each preserve a distinctive elemental identity while making self, ally, enemy, and neutral relationships identifiable.
3. The relationship cue is integrated with the effect composition and cannot reasonably be mistaken for a second gameplay radius.
4. The thin exact boundary remains geometry-faithful and legible on representative production maps.
5. Normal, reduced, bloom-disabled where supported, and primitive variants preserve mandatory geometry and relationship readability.
6. VfxLab can display all four relationship variants using the same production resolver and materializers.
7. Existing authority, role isolation, lifecycle reconstruction, cleanup, and capacity tests remain green.
8. Native captures cover at least Fire and Cold on contrasting production surfaces plus an overlapping relationship scenario, followed by user playtest feedback.

## Verification

- Focused resolver/material/lifecycle tests for all relationship variants and neutral fallback.
- Client and headless/server feature checks to confirm presentation dependencies remain client-only.
- Production-versus-VfxLab parity captures using identical semantic inputs.
- Native normal/reduced/primitive matrix on representative light and dark map surfaces.
- Dense overlap check with self, ally, and enemy effects.
- Record feedback disposition and learn-from-errors notes before completion.

## Non-goals

- Changing team authority, field ownership, damage membership, or network semantics.
- Reworking unrelated fighter selection rings or HUD team colors.
- Building unrestricted per-effect shader graphs.
- Making the editable VFX Workbench part of this ticket.

## User ownership correction — 2026-09-01

The effect relationship contract does not add a self-owned treatment to weapon, ultimate, projectile, or area VFX. The player's existing fighter-selection ring is the self-identification channel. Effect recipes need integrated ally and enemy treatments plus a neutral fallback; they must not render the current generic green self annulus. Any earlier scope or acceptance wording listing four effect relationship variants is superseded by this correction. VfxLab should preview ally, enemy, and neutral effect contexts through the production resolver.

## Implementation findings and first slice — 2026-09-01

Live source inspection identifies the reported overlay in `ElementalAreaStyle::spawn_relationship`: a complete relationship-colored `area_ring` at 90% radius and higher elevation, named `Elemental area relationship inset`. It is independent from the player's `FighterGroundMarker3d`; the player-selection ring remains untouched.

Implement the relationship language as effect-owned exact-boundary punctuation:

- Collapse `SelfOwned` to the ordinary friendly/ally effect treatment. There is no green self effect channel; the controlled fighter ring remains the self-identification channel.
- Neutral fields add no relationship punctuation.
- Ally/friendly fields add four restrained tangential arc segments directly on the exact gameplay boundary.
- Enemy fields add eight inward-pointing chevrons directly on that same boundary.
- Color remains a secondary channel (production ally blue / enemy red); arc versus chevron silhouette and four-versus-eight cadence prevent hue-only dependence.
- Remove the full 90%-radius inset. Preserve the elemental exact ring, fill, shader, accents, authoritative radius, lifecycle, and timing.
- Apply the boundary punctuation to Cold, Fire, Poison, and Restoration area styles in normal, reduced, primitive, and reduced-primitive variants. Keep counts fixed and bounded.
- Collapse `SelfOwned` to the ally material for elemental projectile relationship cores so weapon effects also stop using the green self color; no gameplay signature or protocol migration is required.
- Extend VfxLab with a bounded `neutral|ally|enemy` relationship selector and interactive relationship cycle. Fixture facts must drive the existing production observer/signature path rather than injecting a Lab-only overlay. Evidence/guide output must name the selected and resolved relationship.

The first native review will use the neutral diagnostic floor for Fire and Cold across ally/enemy and normal/reduced/primitive modes. Production-map surface comparison and paired gameplay capture remain sequenced under BRL-0108; BRL-0107 will still verify one production gameplay relationship path before closeout.

## Implementation and verification evidence — 2026-09-01

Implemented the integrated relationship language through the existing production signature and lifecycle materializers:

- Removed `Elemental area relationship inset`, the independent full annulus at 90% radius.
- Added fixed-world-size exact-boundary punctuation shared by Fire, Cold, Poison, Restoration, and circular Splash reuse: four translucent calm blue lenses for ally/`SelfOwned`, eight opaque hot-red inward chevrons for enemy, and no mark for neutral.
- Kept the marks anchored at the authoritative radius with unit scale. The elemental fill, exact bright boundary, palette, material animation, and accent silhouette remain independently owned by the elemental recipe.
- Made the flat relationship meshes explicitly two-sided so camera composition cannot cull the cue.
- Replaced the projectile `SelfOwned` green material selection with the ally relationship material; fighter self-identification remains the existing controlled-fighter selection ring.
- Added VfxLab `--relationship neutral|ally|enemy` and interactive `O` cycling. The Lab creates real owner/team/observer facts, then uses the production observer-relative signature resolver and retained field materializer; capture JSON records both the requested context and resolved signature.
- Updated the durable presentation contract and VfxLab documentation.

Automated evidence:

- `cargo test --locked --no-default-features --features client --lib relationship -- --nocapture`: 4 passed, including self-to-ally collapse, neutral fallback, Lab production observer inputs, exact-radius anchors, constant scale, and inward orientation.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `cargo test --locked --no-default-features --features client --all-targets`: 645 library tests plus client binary/public-API targets passed.
- `./scripts/check-server-features.sh`: passed; server graph excludes client presentation capabilities.
- `./scripts/check-v3-world-presentation.sh`: passed; no second gameplay-world renderer was introduced.

Native VfxLab evidence was reviewed at radius 72 and radius 150 for Fire ally/enemy, Cold enemy, Fire neutral, reduced Fire enemy, and primitive Cold ally. Paired PNG/JSON artifacts are under `target/brl-0107-*`. The review caught and corrected two rejected iterations: radius-proportional marks became dominant on large fields, and one-sided flat meshes were culled. The final constant-size, two-sided cues remain identifiable without bloom and survive reduced/primitive modes. Production-map surface and overlap captures remain intentionally assigned to blocking follow-up BRL-0108, per this ticket's implementation sequencing.

Ticket remains in `doing` pending the requested native user review of ally/enemy/neutral relationship readability. Feedback and learn-from-errors disposition will be recorded before closeout.

## Rejected native review and redesign requirement — 2026-09-01

User review rejects the first exact-boundary punctuation pass:

- The eight red triangles still read as a generic ownership overlay, not as a natural part of the Fire/elemental design. Replacing an annulus with discrete overlay geometry does not satisfy the effect-owned visual-language outcome.
- The Restoration area shader's four orthogonal center strokes can form an accidental swastika-like symbol. This motif is unacceptable and must be removed, including normal, reduced, and primitive comparisons where the composition can recur.

Disposition: keep BRL-0107 in `doing`; discard the triangle/arc relationship meshes as the production design. Ownership must modulate native per-style layers—Fire motion/heat bands, Cold crystals/frost, Poison contamination/bubbles, Restoration vitality/ripples—through a bounded shared relationship parameter or style-owned material variants. The result must not add freestanding symbols, cardinal spokes, a second boundary, or geometry that remains recognizable when the elemental layer is mentally removed. Replace Restoration's orthogonal center composition with an asymmetric or continuously curved vitality motif that cannot produce hooked-cross or cross-like negative space. Repeat native review before closeout.

## User clarification: type pattern plus team color accent — 2026-09-01

The approved redesign principle is: the effect's pattern and motion communicate semantic type, while a restrained color accent within that existing pattern communicates team relationship. Fire remains flame/heat, Cold remains crystal/frost, Poison remains contamination/bubbles, and Restoration remains curved vitality/ripple motion. Ally uses the production ally-blue accent, enemy uses production enemy-red, neutral has no team accent, and `SelfOwned` uses ally because fighter selection owns self identification. Team color must tint or highlight native filaments, crystals, bubbles, petals, or ripples; it must not add an independent symbol, ring, triangle, or other ownership overlay.

## Implemented redesign after native feedback — 2026-09-01

The rejected ownership-overlay design has been removed. Persistent elemental areas no longer spawn a second relationship ring, arrow, chevron, or center icon. The corrected composition has two independent semantic channels:

- effect type is carried by the effect-owned pattern and motion: Cold crystals, Fire flame filaments/tongues, Poison contamination cells and bubbles, and Restoration curved vitality ripples/petals;
- team ownership is carried only by a restrained color accent applied to selected parts of that existing pattern: ally/self blue, enemy red, neutral untinted.

Ally and enemy use the same masks, cadence, silhouette, and animation. Changing relationship changes color only. This prevents relationship from becoming a second type language and keeps simultaneous overlapping effects comparable. `SelfOwned` intentionally uses the ally color treatment because the controlled fighter already owns a separate self-selection cue.

Normal Fire, Poison, and Restoration use three bounded material variants (neutral, ally, enemy) whose shader parameter selects the color accent. This permits simultaneous differently owned fields without mutating one shared material. Cold and degraded variants tint their existing native mesh accents at the same bounded cadence for ally and enemy. The exact gameplay boundary remains type-owned and does not become a relationship overlay.

The Restoration cardinal center motif reported as resembling a prohibited hate symbol was removed completely. It is replaced by an asymmetric, offset, curved five-fold vitality ripple with no cardinal strokes or icon-like negative space.

The VfxLab relationship selector and guide now expose this contract directly. `O` cycles Neutral, Self, Ally, and Enemy while `C` changes elemental type, so type and ownership can be compared independently at a fixed deterministic sample tick.

### Verification evidence

- `cargo test --locked --no-default-features --features client --lib 'client::presentation_3d::combat::elemental_style::tests' -- --nocapture` — 5 passed; explicitly checks bounded material parameters and identical ally/enemy pattern cadence.
- `cargo check --locked --no-default-features --features client --all-targets` — passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — passed.
- `cargo test --locked --no-default-features --features client --lib` — 645 passed, 0 failed.
- `./scripts/check-server-features.sh` — passed; server feature graph remains presentation-free.
- `./scripts/check-v3-world-presentation.sh` — passed.
- scoped `git diff --check` — passed.
- matched deterministic Fire captures at tick 45 confirm the same silhouette and filament placement with only the relationship color changed:
  - `target/brl-0107-fire-ally-color-only-v1/vfx-lab-elemental-field---area-000001-1788298877436-000.png`
  - `target/brl-0107-fire-enemy-color-only-v1/vfx-lab-elemental-field---area-000001-1788298882269-000.png`
- additional visual review covered neutral Fire, ally/enemy Fire, ally/enemy Cold, ally/enemy Poison, ally/enemy Restoration, reduced Fire, and primitive Cold. The ticket remains `doing` pending native user review of the corrected normal presentation.

## Closeout and accepted native review — 2026-09-02

The user accepted the corrected type-pattern/team-color presentation after reviewing the matched VfxLab path. This acceptance supersedes the original proposal for hue-independent relationship geometry: pattern and motion identify the effect family, while relationship intentionally uses only the restrained blue/red accent inside that pattern. No freestanding ownership shape remains.

Feedback disposition is complete:

- dominant green self annulus — removed; self effects use the ally accent and the fighter selection ring continues to identify self;
- red ownership triangles/blue arc overlays — rejected and removed rather than iterated further;
- accidental cross/hooked-cross Restoration composition — removed and replaced with asymmetric curved ripples;
- final native-pattern color modulation — accepted.

Production-map selection and overlap tooling/captures remain deliberately owned by BRL-0108, which this ticket now unblocks. That linked ticket supersedes the original BRL-0107 production-map/overlap capture criterion; BRL-0107 owns the visual-language contract and production materialization, while BRL-0108 owns real-map comparative presentation evidence.

### Learn-from-errors review

The initial redesign changed the shape of the overlay without changing its ownership: a ring became repeated triangles/arcs, but it remained independent geometry and therefore still looked pasted on. The prevention rule is to test whether a relationship cue remains recognizable after mentally removing the elemental effect; if it does, it is probably still an overlay. Future effect-relationship work should begin by parameterizing native type layers, then compare neutral/ally/enemy at the same deterministic tick so geometry drift is immediately visible. Symmetric cardinal motifs also require an explicit symbol-safety review before native handoff. These lessons are now captured in the durable presentation contract and VfxLab guide.
