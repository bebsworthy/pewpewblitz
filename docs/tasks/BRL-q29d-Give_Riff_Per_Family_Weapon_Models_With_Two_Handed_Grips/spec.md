# Outcome

Each weapon family reads at gameplay scale from the weapon in Riff's hands.

# Context

BRL-57hs shipped Riff with `combat_idle_<family>` / `attack_<family>` clips for `1h`, `heavy`,
`throwable`, and `melee`, but every family still attaches the shared Kenney `blaster-a` to
`socket_weapon_r`. Two-handed families only approximate the off hand; there is no left-hand IK,
reload clip, or weapon compatibility check (asset_src/characters/riff/spec.md section 34).

# Scope

- Original procedural weapon GLBs per family (or an accepted curated source), built and validated
  like Riff (`asset_src/`, `just` recipe, `assets/brawler/models/original/`).
- Client selection of the weapon model from the replicated primary weapon delivery family.
- Off-hand placement on two-handed weapons (authored poses or bounded IK).
- Optional reload clips if the economy exposes a reload state.

# Out of scope

Per-brawler character selection, cosmetics.

# Acceptance criteria

- Every shipped weapon preset shows its family weapon in gameplay and the Dashboard preview.
- Primitive fallback unchanged; outline hull cap respected.
- Native captures on all three themes and owner visual acceptance.
