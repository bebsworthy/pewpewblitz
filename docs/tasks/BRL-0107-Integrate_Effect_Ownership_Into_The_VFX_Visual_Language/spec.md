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
