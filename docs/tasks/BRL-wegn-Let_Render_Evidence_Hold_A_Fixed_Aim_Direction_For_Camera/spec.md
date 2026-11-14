# Outcome

`scripts/v3-render-evidence.sh` can hold a fixed aim direction for the measured client, so native
captures show the BRL-vptw camera look-ahead toward the top, bottom, and sides of the screen.

# Context

- BRL-vptw made the aim lean grow with the commanded reach and the view's depth in the aim direction
  (`presentation_3d/camera_rig.rs`, `assets/catalogs/camera_rig.ron` schema 3). Its pure tests prove
  the fighter safe frame and the reach coverage, but its acceptance also asks for native captures.
- The release evidence client has no fixed-aim input. Automation's `aim_axis` exists only in the
  `automation` feature build, and render evidence captures the Pulse Sidearm only (BRL-1fam owns
  weapon selection).

# Scope

- An opt-in evidence-only aim direction (e.g. `BRAWLER_RENDER_AIM=up|down|left|right`) applied
  through the canonical local aim sample, not a second gameplay path.
- Combine with BRL-1fam's weapon selection for long-range families (Sticky Blomb, Arc lob, Splash).

# Acceptance criteria

- Paired PNG/JSON captures show the lean for each direction, and the JSON records the aim sample.
- No player-facing input or gameplay change; default evidence runs unchanged.
