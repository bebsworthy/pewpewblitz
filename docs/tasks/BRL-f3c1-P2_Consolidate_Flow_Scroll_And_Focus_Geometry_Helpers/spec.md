# Scope

Extract only the proven pure geometry and style helpers shared by client-flow screens.

# Acceptance

- One helper normalizes wheel delta with a caller-supplied multiplier.
- One helper clamps offset to content/viewport bounds.
- One helper computes the minimal offset keeping a focused interval visible.
- Focus policy, markers, layout constants, gating, and render trees stay screen-specific.
- Preserve or convert per-screen coverage to focused helper plus integration tests.
- Client UI tests and native wide/compact behavior pass.

# Constraints

Coordinate with ARCH-01. Do not introduce a generic widget hierarchy or erase screen customization.

# As built

Implemented one private `client::flow::screens::scroll` owner with exactly three shared operations:

- `normalized_wheel_delta` consumes the frame's wheel messages while retaining each caller's
  `24.0` or `36.0` line multiplier;
- `clamp_scroll_offset` bounds logical offsets with Bevy's scale-aware content overflow and treats
  zero computed extents as layout-not-ready rather than erasing an early input;
- `offset_keeping_interval_visible` makes the smallest logical offset adjustment that contains the
  focused interval and explicitly converts physical layout geometry with the node inverse scale.

Dashboard, game selection, brawler list/details, and weapon equipment retain their own markers,
state/overlay gates, focus lookup, padding, line multiplier, layout, and render tree. No generic
widget hierarchy or shared style policy was introduced.

# Research

- `references/bevy/examples/ui/scroll_and_overflow/scroll.rs` is the local Bevy example establishing
  `(content_size - size) * inverse_scale_factor` as the logical scroll range.
- The installed Bevy 0.19.1 `bevy_ui/src/ui_node.rs` documents `ComputedNode` extents as physical
  pixels and `inverse_scale_factor` as the physical-to-logical conversion.
- The installed Bevy 0.19.1 `bevy_ui/src/layout/mod.rs` independently confirms that layout clamps
  `ScrollPosition` to the computed overflow range.

The exact supported dependency API was available locally, so internet research was unnecessary.

# Verification

- Three focused pure-helper tests pass for mixed line/pixel messages, scaled lower/upper bounds,
  layout-not-ready behavior, already-visible focus, focus above/below the viewport, and scaled
  focus geometry.
- Existing equipment and long game-catalog scroll integration tests pass unchanged.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`
  passes.
- `cargo test --locked --no-default-features --features client --all-targets` passes: 418 library
  tests and the client binary target, with no failures.
- Native interactive product-shell checks pass on the exact implementation:
  `target/brl-0015-bundle-native-20260828/compact-dashboard.jpeg` shows the default Play focus fully
  visible in the compact `640x360` layout, while
  `target/brl-0015-bundle-native-20260828/wide-dashboard.jpeg` preserves the complete wide
  `1280x720` hierarchy. These are visual behavior checks, not performance claims; a background
  render-report attempt was excluded because macOS UI automation throttled its sample rate below
  the locked performance threshold.
