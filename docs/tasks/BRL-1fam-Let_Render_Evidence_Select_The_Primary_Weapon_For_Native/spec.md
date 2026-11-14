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

# Implementation (2026-11-14)

- Root cause beyond the context above: the automation default brawler was created only for an
  empty profile, and the saved-brawler model has no weapon edit (`BrawlerEdit` carries name,
  ultimate, passives only), so a persisted profile kept its first weapon forever.
- `packages/brawler-client/src/profile/automation.rs` (new, `automation` feature only): pure
  `plan_automation_brawler(requested, advertised catalog, snapshot)` returns the next ordinary
  profile command. No request: seed an empty profile only (historical default, existing selection
  untouched). With `--weapon-preset`: nothing if the selected brawler already uses that weapon;
  otherwise `SelectBrawler` on the earliest-created brawler with that weapon; otherwise
  `CreateBrawler` with that weapon (first advertised fighter/ultimate, first two selectable
  passives), then the select on the next snapshot; `Unavailable` (error + `AppExit::error`) for an
  unadvertised weapon or a full profile. The system caps automation at 4 profile requests so a
  rejected recipe fails instead of looping. The server still validates the recipe and resolves
  the match loadout from the selected saved brawler: no client-side loadout path.
- `queue/automation.rs`: Practice/queue automation waits until `automation_brawler_ready` (the
  selected brawler carries the requested weapon) before requesting a match.
- `src/config.rs`: `--weapon-preset` is validated against the costed weapon IDs in the embedded
  `content/catalogs/builds.ron` (`weapon_costs`) instead of the literal 1..=6, so Splash (7) and any
  future costed weapon are accepted; the error lists the valid IDs. CLI usage text updated.
- `scripts/v3-render-evidence.sh`: `BRAWLER_RENDER_WEAPON=<ID>` (digits, no leading zero; the
  client does the catalog check) adds `--weapon-preset` to the measured client only, via
  `measured_weapon_flags` beside `measured_combat_flags`, so BRL-wegn can add a parallel
  `BRAWLER_RENDER_AIM` flag set the same way. Unset leaves default runs unchanged.
- Docs: README executable-configuration bullet, docs/11 render-evidence paragraph.
- No server, protocol, catalog, fingerprint, or player-build change.

# Verification (2026-11-14)

Scratch `CARGO_TARGET_DIR` throughout; the user's server on UDP 5000 was not touched.

- Focused: `cargo test --locked -p brawler-client --features automation --lib -- profile::automation
  headless_weapon_preset` 4 passed (new: default run seeds only an empty profile and keeps an
  existing selection; requested weapon seeds/creates/selects-earliest/no-op and the queue gate;
  unadvertised weapon or full profile is `Unavailable`; CLI bound accepts every costed weapon
  including Splash and rejects 0 and max+1).
- `just check`: pass. `just lint` (VS Code's bundled ripgrep first on PATH): pass, including
  `_client-features` (player build excludes automation). `just test`: pass, 2368 passed, 0 failed.
- Native (patched scratch copy of the script: scratch target binaries, supervisor
  `--data-directory` in the scratchpad, bind 127.0.0.1:5187; `BRAWLER_RENDER_PRACTICE=1
  BRAWLER_RENDER_COMBAT=1`, warmup 5 s, measure 10 s). Two runs on the same data directory so the
  second proves a persisted selection is overridden:
  - `BRAWLER_RENDER_WEAPON=2` (Scatter Cannon, fresh profile): log "automation creating saved
    brawler weapon_base_id=2"; p50/p95/p99 8.332/8.585/8.682 ms, result=pass; captures show the
    five-lane Scatter fan with five projectiles; JSON `source_preset_id` 2.
  - `BRAWLER_RENDER_WEAPON=7` (Splash, profile already selecting Scatter): logs create
    weapon_base_id=7 then select; 8.326/8.599/8.916 ms, result=pass; captures show the Splash lob
    guide, landing circle, and splash VFX; JSON `source_preset_id` 7.
  - Screenshots (PNG + JSON, frames 300/480/660):
    `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1fam/weapon-2.txt.captures/`
    and `.../scratchpad/brl1fam/weapon-7.txt.captures/`.
- Not run natively this pass: Arc Launcher (3), Impact Blade (4), Sticky Blomb (5), Spray (6).
  The selection path is weapon-generic (catalog lookup, no per-weapon branch) and unit-tested, and
  BRL-2dv1 covers each family's lane geometry; a reviewer wanting the full listed matrix can run
  `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1 BRAWLER_RENDER_WEAPON=<3..6>`.
- BRL-tpd8 was needed to run safely beside the live server (scratch target and data dir); it was
  worked around with a patched scratch script copy, not implemented here.

# Playtest

None needed: this is evidence tooling behind the `automation` feature with no player-facing change.

# Follow-ups

- BRL-583f: log "headless movement automation enabled" once (1,773 lines in one Practice run).
- BRL-hkbm: keep the "Capture saved" toast out of scheduled render-evidence frames (it covers the
  bottom-right HUD in the 480/660 captures).
- Splash's landing circle sits at the right screen edge with `--aim-axis 1,0`; BRL-wegn (fixed aim
  direction) plus BRL-vptw framing own that.

# Learn from errors

- The ticket context named the old `src/client/profile.rs` path; the client moved to
  `packages/brawler-client` (BRL-0170). Check paths against the tree before planning.
- The first idea (edit the selected brawler's weapon) does not exist in the profile model; reading
  `BrawlerEdit` before designing avoided a protocol change. Prefer the existing create/select
  commands over new wire surface for automation needs.
- `cargo test` filters and zsh: `grep --include=*.rs` fails under zsh globbing; quote patterns.


# Closeout (2026-11-14)

Closed without a user playtest (automation-only tooling). Native runs covered Scatter Cannon (2) and Splash (7); Arc Launcher, Impact Blade and Spray were not run natively. Accepted because weapon selection has no per-weapon branch and is unit-tested across every costed catalog weapon; any family can be captured on demand with BRAWLER_RENDER_WEAPON=<ID>.
