# Outcome

Camera shake strength fits each weapon's weight and the player's preference.

# Context

BRL-0143 added a presentation-only camera rig (`src/client/presentation_3d/camera_rig.rs`, tuning in `assets/catalogs/camera_rig.ron`). Its shake channel uses one authored local-fire trauma for every weapon and one local-damage trauma regardless of amount; the only player control is the existing reduced-motion / reduced-combat-effects toggles, which disable shake entirely. Barrel blast damage to the local fighter only shakes through the distance-scaled explosion trigger because environment damage emits legacy cues that `DeduplicatedCombatCue` filters out.

# Scope (decide from BRL-0143 playtest feedback)

- Optional per-weapon (or per delivery family) fire-trauma scaling from authored presentation data, not gameplay catalogs.
- Optional damage-taken trauma scaled by damage amount relative to max health.
- A camera shake intensity setting (for example Off / Low / Full) using the existing `ClientShellSettings` persistence and settings UI pattern.

# Out of scope

Camera angle/FOV (BRL-0148); gameplay changes.

# Acceptance criteria

- Shake stays zero under reduced motion and reduced combat effects.
- Settings persist and migrate through the existing settings validation.
- Focused tests plus a native playtest.
