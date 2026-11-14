# Outcome

`scripts/v3-render-evidence.sh` can capture the aim lane, projectiles, and effects of any primary
weapon family (Straight, Spread, Lobbed, MeleeArc, StickyStraight, ConeSpray, Splash).

# Context (found during BRL-2dv1)

- The client's `--weapon-preset <1-6>` only seeds a brawler when the account's saved-brawler
  snapshot is empty (`src/client/profile.rs`), so a Practice render run after the first keeps
  the existing Pulse Sidearm brawler. A local, uncommitted script variant passing
  `--weapon-preset` produced Pulse captures for presets 2-6.
- The flag's 1-6 bound rejects preset 7 (Splash), so Splash cannot be selected at all.
- The script has no environment variable for a weapon choice.

# Scope

- A validated `BRAWLER_RENDER_WEAPON` (or equivalent) that reliably selects the measured
  client's primary weapon for the run (fresh profile or explicit selection), covering all
  embedded presets including Splash.
- Extend the CLI bound to the catalog instead of a literal.

# Acceptance criteria

- One render run per weapon family shows that weapon in its captures and HUD.
- Existing default runs are unchanged.

# Verification

Script runs for at least Scatter, Arc Launcher, Impact Blade, Spray, and Splash.
