# Closeout specification (rewritten 2026-11-20)

The original 795-line implementation spec (2026-08-31) is preserved in git history of `docs/tasks/BRL-hheg-*/ticket.md`. Its implementation was delivered by child tickets; this spec records only the closeout.

## Delivered by
BRL-vtnc (contract), BRL-kfdm (VfxLab viewer), BRL-6akb (layered mesh recipes), BRL-4vph (signatures across lifecycle adapters), BRL-rxx0 (Cold/Fire), BRL-yqpx (Hanabi particle adapter + CPU fallback), BRL-qxdw (lobbed/explosion), BRL-b6hr (remaining families), BRL-pykq (relationship language), BRL-h270 (team-coloured projectiles). The held-weapon seam went to BRL-q29d. All done.

Current contract: `docs/11-art-and-presentation-direction.md` (VFX sections), `packages/brawler-client/src/vfx/{catalog,recipes,signature}.rs`, `assets/catalogs/vfx.ron` (schema 8).

## Owner decision (2026-11-20)
Drop the BRL-08sz blocker. Close with a native match capture plus VfxLab captures on a real map theme instead of building the VfxLab map preview first. BRL-08sz stays an independent tooling ticket.

## Closeout evidence (2026-11-20)
- `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1 BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_GAME_TYPE=wipeout-3v3 just v3-render-evidence target/hheg-closeout/practice-3v3.txt`: three paired captures written. A live Restoration field, lobbed aids, Splash roots, status roots and 23 particle emitters were active with no evictions or suppression (asset `practice-3v3-live.png`). The report's frame-time gate failed (p95 43 ms) at load average 19; that gate is unrelated to this closeout and not re-run.
- VfxLab `--theme 1 --relationship ally|enemy` pairs for straight/fire, field/cold, lobbed/neutral, splash, cone/poison (asset `relationship-sheet.png`).

## Acceptance criterion 9 (relationship readability): partly met
- Projectiles: met. Bodies are team blue or red.
- Areas and cones: not met at a glance. Ally and enemy differ only by a few small pattern highlights; an enemy Cold field reads blue. Deferred to BRL-nhs7.

## AAA quality gate
Re-baselined against the toon/HDR/bloom direction adopted in the 2026-10-23 review (BRL-farz retuned VFX shaders for it). No further sign-off is owned by this ticket; subjective polish goes to new tickets from playtests.

## Learning
Umbrella specs written ahead of their children go stale fast. Close the umbrella when its last child closes, and keep the current contract in `docs/11`, not in the umbrella spec.
