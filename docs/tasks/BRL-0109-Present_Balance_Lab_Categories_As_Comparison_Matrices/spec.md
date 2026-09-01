# Outcome

Make side-by-side comparison the primary Balance Lab workflow. Within a gameplay category, subjects are columns and comparable tunable properties are rows, so an operator can inspect and edit tradeoffs without repeatedly switching subjects.

# Inspection findings

- The current desktop editor is category -> selected subject -> vertical field rows. Only one subject's values can be seen at a time.
- At 1280x720 with the roster collapsed, the tuning workspace begins about 469 px down the page; the nine-row Default fighter editor produces a roughly 1,759 px document. The visible viewport shows the category/subject navigation and group heading before it shows useful values.
- Fighters are a homogeneous 3-column matrix: Default, Lightweight, and Reinforced each expose the same nine fields in the same four groups.
- Weapons, ultimates, passives, world objects, and modes are heterogeneous. A useful matrix must align explicitly comparable fields and render unavailable cells as absent; it must not infer gameplay semantics from serialized paths or display strings.
- The server-provided editor manifest already owns path, section, subject, group, label, unit, scale, bounds, step, control, and help. Draft mutation, validation, apply/revert/restore, and canonical-difference behavior can remain unchanged.
- Passives currently arrive in the Ultimates editor section. A comparison-first category model should not mix those subject families in one very wide matrix.

# Proposed interaction

## Category workspace

Keep the top-level category navigation, but replace the subject sidebar and single-subject panel with a comparison matrix.

For Fighters:

| Property | Default | Lightweight | Reinforced |
|---|---:|---:|---:|
| Maximum health | [1000] | [900] | [1200] |
| Movement speed | [70] | [90] | [60] |
| Health recovery rate | [100] | [90] | [120] |

Group headings such as Core stats, Recovery, Concealment, and Elemental baselines remain as full-width separators. Put the gameplay unit in the row heading so it is not repeated in every cell.

For heterogeneous categories, build the union of explicitly declared comparison rows. Align fields that share the same server-owned comparison identity; show an em dash for a subject that does not support that row. Keep every editable field represented exactly once. Horizontally scroll the matrix when all subject columns do not fit, with sticky row headings and sticky subject headers.

Expose Passives as its own category rather than treating them as Ultimate subjects. Global, World objects, and Modes may use the same matrix with one or a few columns; do not maintain a second desktop editor paradigm for those sections.

## Cell editing and detail

- Each populated cell contains a compact exact numeric input for the draft value.
- Preserve two independent state signals: unapplied change and difference from canonical server default. Use restrained cell borders/background markers plus a visible legend; do not rely on color alone.
- Focusing a cell presents a compact field detail strip or inspector containing Applied, Default, valid range, help text, Reset, and the range control when the descriptor prefers range-and-number.
- Server and client validation errors attach to the exact cell, update the selected category if necessary, and move focus to that cell.
- Native table semantics, scoped row/column headers, descriptive labels, keyboard tab order, and visible focus are required.
- The bottom Apply/Revert/Restore action bar retains its current authoritative behavior.

## Page density

Compact the page header and secondary summaries so comparison data appears earlier. Keep Players & loadouts collapsible and make its collapsed state the space-efficient tuning posture. The canonical-difference summary may become a compact toolbar/status row, but Copy differences and its current semantics remain.

At 1280x720 with loadouts collapsed, the fighter matrix header and at least four value rows should be visible above the fixed action bar without scrolling the page. At narrow widths, use a deliberate stacked single-subject fallback or a horizontally scrollable accessible grid; no control may overflow or become unusably small at 390x844.

# Manifest contract

Extend the non-persisted HTTP editor manifest with stable, server-authored comparison metadata rather than deriving alignment from paths or labels:

- a stable comparison row key for each descriptor;
- a distinct Passives section/category;
- manifest ordering remains the source of category, group, row, and subject order.

Descriptors sharing a comparison row key in one category must have compatible semantics and units. Add server tests for duplicate subject/row pairs and incompatible aligned descriptors. Bump the editor-manifest schema version. This does not change the persisted tuning snapshot, gameplay protocol, or authoritative apply transaction.

# Frontend ownership

Keep this change inside the existing `pages/balance-lab` slice:

- the page/controller continues to own server state and apply/revert/restore orchestration;
- a pure `lib` transformation builds category subjects, groups, rows, and cells from descriptors;
- semantic `ui` components own the comparison workspace, matrix, cell, and focused-field detail;
- reuse the existing editor number conversion and validation helpers;
- avoid a new app-wide table framework or generic design system for this one operator screen.

# Acceptance criteria

- Fighters render as one editable three-column comparison with all nine current properties aligned.
- Every current numeric field in every category remains editable exactly once.
- Weapons and abilities align only server-declared comparable rows; unsupported subject/row intersections are visibly absent.
- Passives are no longer presented as Ultimate subjects.
- Editing preserves current display/storage conversion, authoritative bounds, inline validation, changed counts, canonical-difference counts, apply, revert, per-field reset, restore-defaults, and server-error focus behavior.
- Applied and canonical-default differences remain distinguishable without relying only on color.
- Sticky headers keep row and subject identity visible during vertical/horizontal scrolling.
- The 1280x720 density target and 390x844 usability target pass browser inspection.
- No rendering or client dependency enters the headless server feature graph.
- The Balance Lab operator guide describes the matrix workflow and comparison-state legend.

# Verification

- Focused pure tests for descriptor-to-matrix transformation, stable ordering, sparse cells, duplicate detection, and complete field coverage.
- Existing editor conversion, canonical-difference, and player-loadout tests remain green.
- Server editor-manifest tests cover comparison keys, Passives categorization, compatibility, and schema version.
- `npm run --prefix tools/balance-lab-web test`, `npm run --prefix tools/balance-lab-web build`, focused Balance Lab Rust tests, and the canonical role-specific checks proportional to touched server metadata.
- Production-build browser passes at 1280x720 and 390x844 using an actual server-shaped state; test editing, reset, validation focus, horizontal/vertical scrolling, canonical markers, and Apply/Revert/Restore.
- Native `just balance-lab` playtest confirms the comparison improves real tuning decisions and that no editable axis disappeared.

# Out of scope

Charts, derived power scores, balancing recommendations, multi-cell paste/fill, undo history, live/hot apply, canonical-content export, and changes to gameplay values or validation ceilings.



# Implementation evidence — 2026-09-01

## Delivered

- Added editor-manifest schema 11 with server-authored `comparisonKey` metadata, duplicate-cell and compatible-row validation, and a distinct Passives section.
- Added the pure page-local descriptor-to-matrix transformation and focused tests for ordering, sparse cells, duplicate rejection, incompatible rows, and complete field coverage.
- Replaced the single-subject vertical editor with an accessible table: subjects are columns, properties are rows, group separators remain visible, unsupported cells show an em dash, exact numeric inputs remain inline, and focused-field details retain applied/default/range/help/slider/reset behavior.
- Preserved the existing controller and authoritative apply/revert/restore transaction path. Edited and canonical-default differences have independent cell markers and a textual legend.
- Collapsed the roster by default, compacted the tuning header, separated Passives from Ultimates, and updated the operator guide.

## Verification

- `npm run --prefix tools/balance-lab-web test`: 15 passed.
- `npm run --prefix tools/balance-lab-web build`: passed (`tsc --noEmit` and Vite production build).
- `cargo check --locked --no-default-features --features balance-lab --all-targets`: passed.
- `cargo clippy --locked --no-default-features --features balance-lab --all-targets -- -D warnings`: passed.
- `cargo test --locked --no-default-features --features balance-lab --all-targets`: 550 unit tests plus the affected binary/integration targets passed. Expected error logs came from negative-path tests.
- Focused editor-manifest suite: 23 passed before the final responsibility-only helper extraction; the post-extraction strict all-target Clippy compilation passed. A redundant post-extraction focused test link could not complete because the local volume reported `No space left on device`; no user build artifacts were deleted to retry it.
- `git diff --check`: passed.

## Browser evidence

Production assets were exercised against a temporary server-shaped schema-11 state.

- At 1280x720, Fighters showed all three columns and five value rows above the fixed action bar; edit, non-default, combined markers, focus details, Reset, and Apply enablement behaved correctly.
- Weapons preserved sparse intersections with an em dash, and Passives appeared as a separate category.
- Invalid input attached to the exact cell, exposed `aria-invalid` and an error description, and disabled Apply. Browser error/warning logs were empty.
- At 390x844, document width remained 390 px, the matrix began at 379 px, the 108 px action bar began at 736 px, and four fighter rows were visible. The matrix scrolled from 368 px to 626 px horizontally while sticky property and row headings remained at the matrix left edge.

## Remaining acceptance evidence

The native `just balance-lab` subjective playtest is still required before moving BRL-0109 to done. Requested observation: whether side-by-side rows make real fighter and weapon tradeoffs materially easier to judge, and whether any editable axis feels missing or awkward.

## Learn-from-errors review

The first narrow pass retained desktop-sized summary cards and a 193 px action bar, leaving no matrix values visible. The corrected breakpoint compacts collapsed summaries and the action bar, and the acceptance check now measures element positions and scroll widths rather than relying on a screenshot alone. Future comparison/editor work should validate the smallest target viewport before polishing secondary summaries.


# Implementation evidence — 2026-09-01

## Delivered

- Added editor-manifest schema 11 with server-authored `comparisonKey` metadata, duplicate-cell and compatible-row validation, and a distinct Passives section.
- Added the pure page-local descriptor-to-matrix transformation and focused tests for ordering, sparse cells, duplicate rejection, incompatible rows, and complete field coverage.
- Replaced the single-subject vertical editor with an accessible table: subjects are columns, properties are rows, group separators remain visible, unsupported cells show an em dash, exact numeric inputs remain inline, and focused-field details retain applied/default/range/help/slider/reset behavior.
- Preserved the existing controller and authoritative apply/revert/restore transaction path. Edited and canonical-default differences have independent cell markers and a textual legend.
- Collapsed the roster by default, compacted the tuning header, separated Passives from Ultimates, and updated the operator guide.

## Verification

- `npm run --prefix tools/balance-lab-web test`: 15 passed.
- `npm run --prefix tools/balance-lab-web build`: passed (`tsc --noEmit` and Vite production build).
- `cargo check --locked --no-default-features --features balance-lab --all-targets`: passed.
- `cargo clippy --locked --no-default-features --features balance-lab --all-targets -- -D warnings`: passed.
- `cargo test --locked --no-default-features --features balance-lab --all-targets`: 550 unit tests plus the affected binary/integration targets passed. Expected error logs came from negative-path tests.
- Focused editor-manifest suite: 23 passed before the final responsibility-only helper extraction; the post-extraction strict all-target Clippy compilation passed. A redundant post-extraction focused test link could not complete because the local volume reported `No space left on device`; no user data was deleted to retry it.
- `git diff --check`: passed.

## Browser evidence

Production assets were exercised against a temporary server-shaped schema-11 state.

- At 1280x720, Fighters showed all three columns and five value rows above the fixed action bar; edit, non-default, combined markers, focus details, Reset, and Apply enablement behaved correctly.
- Weapons preserved sparse intersections with an em dash, and Passives appeared as a separate category.
- Invalid input attached to the exact cell, exposed `aria-invalid` and an error description, and disabled Apply. Browser error/warning logs were empty.
- At 390x844, document width remained 390 px, the matrix began at 379 px, the 108 px action bar began at 736 px, and four fighter rows were visible. The matrix scrolled from 368 px to 626 px horizontally while sticky property and row headings remained at the matrix left edge.

## Remaining acceptance evidence

The native `just balance-lab` subjective playtest is still required before moving BRL-0109 to done. Requested observation: whether side-by-side rows make real fighter and weapon tradeoffs materially easier to judge, and whether any editable axis feels missing or awkward.

## Learn-from-errors review

The first narrow pass retained desktop-sized summary cards and a 193 px action bar, leaving no matrix values visible. The corrected breakpoint compacts collapsed summaries and the action bar, and the acceptance check now measures element positions and scroll widths rather than relying on a screenshot alone. Future comparison/editor work should validate the smallest target viewport before polishing secondary summaries.


## Post-cleanup verification correction

After scoped `cargo clean -p brawler` removed only regenerable package build artifacts and restored disk capacity, `cargo test --locked --no-default-features --features balance-lab server::balance_lab::editor::tests --lib` completed successfully: 23 passed. This supersedes the earlier note that the redundant post-extraction run could not link.



# Semantic comparison-key audit — requested 2026-09-01

Audit every current Balance Lab descriptor across Fighters, Weapons, Ultimates, Passives, World objects, Modes, and Global. Compare visible semantics, units, storage kinds/scales, bounds, and authored runtime meaning. Classify each apparent duplicate as: should share one comparison row; correctly separate despite a similar label; or ambiguous and requiring a product decision. Specifically verify repeated Maximum range fields in Ultimates. This audit must not change gameplay values. Record a complete field inventory and actionable findings before any correction.



# Semantic comparison-key audit results — 2026-09-01

## Method and inventory

Audited the actual embedded schema-11 manifest, its descriptor adapters, authored parameter variants, bounds, storage conversion, and representative runtime consumers. No gameplay values or persisted paths were changed.

The manifest contains 215 editable descriptors and 147 currently rendered rows:

| Category | Subjects | Fields | Current rows |
| --- | ---: | ---: | ---: |
| Global | 2 | 7 | 7 |
| Fighters | 3 | 27 | 9 |
| Weapons | 7 | 87 | 53 |
| Ultimates | 11 | 68 | 53 |
| Passives | 7 editable subjects | 12 | 12 |
| World objects | 3 | 13 | 12 |
| Modes | 1 | 1 | 1 |

Per-subject field coverage:

- Global: Cold & Freeze 4; Ultimate charge 3.
- Fighters: Default 9; Lightweight 9; Reinforced 9.
- Weapons: Pulse Sidearm 9; Scatter Cannon 14; Arc Launcher 15; Impact Blade 8; Sticky Blomb 13; Spray 13; Splash 15.
- Ultimates: Dash 6; Sentry 17; Self Cloak 1; Reveal Scan 3; Concealment Field 3; Demolition Strike 2; Cryogenic Field 5; Fire Field 7; Poison Field 7; Restoration Field 5; Big Blob 12.
- Passives: Adrenal Response 3; Close Quarters 4; Quick Cycle 1; Tenacity 1; Cryogenic Insulation 1; Filtered Circulation 1; Heat Shielding 1. Lightweight Frame and Reinforced Frame intentionally have no separate passive parameters because their tunable results live in Fighters.
- World objects: Effect tiles 4; Oil barrel 5; Treasure chest & pickup 4.
- Modes: Heist 1.

Coverage is exact: the existing server tests assert every authored numeric weapon/ultimate leaf is represented once, and the manifest test asserts 215 unique paths.

## Root cause

Fighters and the two damageable world objects use explicit semantic comparison keys. Weapons and Ultimates derive the key from the serialized variant path, and Passives prefix it with the parameter variant. Therefore fields align when their storage schema happens to match, not when they express the same balancing axis. The screenshot's four Maximum range rows are the clearest example.

The compatibility validator correctly prevents incompatible cells, but it cannot detect semantic duplicates under different keys. Corrections should be manifest metadata only: preserve persisted paths and gameplay parameter names, and assign explicit semantic row keys plus normalized row group/label/unit.

## Should share one row

### Ultimates

1. Targeting / Maximum range:
   - Reveal Scan maximum range
   - Concealment Field maximum range
   - Demolition Strike maximum range
   - all four Elemental Field maximum ranges
   - Big Blob maximum throw range
   These are the same targeted placement limit with identical integer/milliunit representation and bounds. Five current keys produce four visible rows; one semantic key should cover eight subjects.

2. Area / Primary effect radius:
   - Reveal radius
   - Concealment Field radius
   - Demolition Strike destruction radius
   - all four Elemental Field radii
   These all bound the primary placed ultimate's affected circle. Bounds may differ by subject, which is valid; representation is compatible. Four current keys should become one row covering seven subjects.

3. Timing / Active effect duration:
   - Self Cloak cloak duration
   - Reveal Scan reveal duration
   - Concealment Field duration
   - all four Elemental Field durations
   Four current keys should become one row covering seven subjects.

4. Secondary projectile properties:
   - Sentry projectile speed/radius/range/flight lifetime
   - Big Blob child speed/radius/range/flight lifetime
   Four corresponding pairs express the same subordinate-projectile axes and use compatible representations.

5. Direct damage per hit:
   - Dash impact damage
   - Sentry projectile damage
   - Big Blob child explosion damage
   These are compatible health damage amounts. Use an explicit per-hit label so the row does not imply total ultimate damage.

### Weapons

1. Economy / Ammo capacity:
   - Magazine capacity
   - Charge capacity
   Normalize the display unit to a neutral count such as uses.

2. Economy / Ammo recovery time:
   - Ammo recovery per round
   - Ammo recovery per charge
   Normalize to recovery time per use.

3. Delivery / Maximum range:
   - Straight range
   - Sticky-straight range
   - Lobbed distance
   - Melee reach
   - Cone-spray reach
   - Splash placement distance
   These are the effective authored reach limit and have identical decimal world-unit representation/bounds. Six current keys should become one row covering all seven weapons.

4. Delivery / Attack angle:
   - Scatter total spread angle
   - Melee arc angle
   - Cone angle
   All are the authored angular width in degrees.

5. Delivery / Travel speed:
   - Straight projectile speed
   - Sticky projectile speed
   - Cone-spray propagation speed
   All control how quickly the attack front reaches range and use the same world-units-per-second representation.

6. Delivery / Projectile radius:
   - Straight projectile radius
   - Sticky projectile radius

7. Delivery / Projectile lifetime:
   - Straight projectile lifetime
   - Sticky projectile lifetime

8. Delivery / Muzzle offset:
   - Straight, Sticky-straight, Lobbed, and Splash muzzle offsets

9. Lob delivery:
   - Lobbed and Splash maximum flight time
   - Lobbed and Splash visual arc height
   - Lobbed and Splash landing clearance

10. Area / Effect radius:
    - payload Area radius used by Arc Launcher and Sticky Blomb
    - Splash circle radius
    Normalize to the radius of the attack's affected area.

11. Area / Maximum targets per application:
    - payload Area maximum targets
    - Cone-spray maximum targets per pulse
    - Splash maximum targets per pulse

12. Area / Pulse interval:
    - Cone-spray damage pulse interval
    - Splash pulse interval

13. Capacity / Maximum active instances:
    - Sticky maximum active blobs
    - Splash maximum active areas
    Normalize the display unit to instances.

### Passives

- Cryogenic Insulation, Filtered Circulation, and Heat Shielding Resistance are the same elemental-resistance percentage axis on three alternative subjects. Their three variant-prefixed keys should become one row.

## Presentation-unit and label defects

1. Ultimate speed fields are displayed as world units, but runtime consumes them as rates:
   - Dash knockback speed
   - Sentry projectile speed
   - Big Blob child travel speed
   Their unit should be world units/s. The generic milliunit descriptor helper currently hides this distinction.

2. Elemental Field effect units are too vague:
   - Damage per tick should use health/tick rather than points.
   - Healing should use health/pulse rather than points.
   - Cold per pulse is already explicit.

3. Percentage passives need scale wording:
   - Close Quarters Near damage and Far damage are total damage scales (115% and 85% by default), not additive damage values.
   - Quick Cycle Refill duration and Tenacity Slow duration are duration scales (60% and 65% by default).
   Labels should say damage scale / duration scale or provide equivalent help.

4. The already-aligned weapon Damage row represents damage per payload application. For Spray and Splash this may recur per pulse, so Damage per application is less misleading than an unqualified Damage label.

## Correctly separate despite similar names or units

- Dash maximum movement distance, targeted-ultimate maximum range, Sentry acquisition range, and secondary-projectile range are different stages of an ability and should not share a row.
- Primary effect radius, Sentry body radius, projectile radius, landing clearance, and Big Blob child explosion radius describe different collision/area roles and should remain distinct.
- Active effect duration, field pulse interval, damage-over-time tick interval, damage-over-time duration, projectile flight lifetime, fuse delay, and spray linger are different clocks.
- Direct health damage, damage per tick, healing, and Cold buildup use different resources or cadence and should remain separate.
- Fighter Cold/Poison/Fire resistance fields are distinct elemental axes on the same subject; merging them would create duplicate cells and erase meaning.
- Close Quarters near/far distances and near/far damage scales are paired thresholds/outputs within one passive and must stay distinct.
- Effect-tile movement multipliers, explosion radius/damage, pickup collection radius/restoration, and durability are different interactions. Oil-barrel and treasure-chest Maximum health are already correctly aligned.
- Heist safe Maximum health remains mode-owned and in a different category; cross-category label equality is not a comparison-key defect.

## One ambiguity to resolve during correction

Sentry Lifetime could join Active effect duration because both express ultimate uptime, but it is a deployable lifetime with a much larger ceiling and different operational lifecycle. Recommendation: keep it as Deployable lifetime unless native balancing shows that a single uptime row is more useful.

Straight/Sticky projectile lifetime and Lobbed/Splash maximum flight time are also both travel clocks, but one is an expiry limit and the other determines a lob trajectory. Recommendation: keep two rows while aligning within each delivery family.

## Audit conclusion

The matrix implementation is complete, but semantic alignment is not. The highest-value correction is explicit semantic keys for Weapons, Ultimates, and elemental-resistance Passives, plus the speed/effect unit fixes above. This can be done without changing gameplay values, persisted snapshot paths, authority, or storage schemas. BRL-0109 should remain doing until these findings are dispositioned and the corrected native matrix is accepted.
