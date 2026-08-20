# Research sources

The following sources informed the initial design baseline. They describe the reference genre and engine capabilities; Brawler should use original content and terminology.

## Local source and example snapshots

The repository contains local upstream material so implementation research can inspect real examples instead of guessing APIs. The snapshots are not all version-aligned:

- `references/bevy/examples/` — Bevy 0.20-dev example index and Rust source across application setup, headless apps, plugins, 3D/orthographic rendering, glTF/animation, assets, input, states, UI, and other engine features. Use these as architectural examples and verify every transferred API against Bevy 0.19.
- `references/lightyear/examples/` — Lightyear 0.29 example packages/workspace, Rust source, READMEs, and feature declarations. This snapshot targets Bevy 0.19; `simple_setup`, `simple_box`, and `avian_2d` are the primary starting points for early milestones, while `network_visibility` demonstrates future interest management and visibility lifetimes.
- `references/lightyear/book/` — the local Lightyear book; begin with `src/SUMMARY.md` and follow the relevant tutorial and concept chapters, including `concepts/advanced_replication/interest_management.md` for future concealment work.
- `references/lightyear/crates/replication/replication/src/visibility/` — exact Lightyear 0.29 immediate and room-based network visibility implementation. Use this resolved source when older book terminology differs.

Agents should inspect these folders before using external examples. Treat them as read-only upstream snapshots and cite exact paths in milestone research logs. For exact APIs, use Lightyear's Bevy-0.19-pinned source and official Bevy 0.19 documentation; use the Bevy 0.20-dev snapshot only after verifying the equivalent 0.19 API.

Architecture research follows this priority: Brawler's gameplay and authority requirements; the local Lightyear 0.29 examples and book; official Bevy 0.19 source/documentation; Bevy-native ECS/plugin/schedule patterns; then general Rust dependency hygiene.

## Selected engine and networking stack

- [Bevy 0.19](https://bevy.org/news/bevy-0-19/) — selected engine baseline, code-first scenes, ECS, and engine development status.
- [Bevy 0.19 ECS API](https://docs.rs/bevy/0.19.0/bevy/ecs/index.html) — version-pinned entities, components, systems, queries, schedules, and resources.
- [Bevy 0.19 plugin API](https://docs.rs/bevy/0.19.0/bevy/app/trait.Plugin.html) and [headless example](https://docs.rs/crate/bevy/0.19.0/source/examples/app/headless.rs) — version-pinned plugin composition and omission of rendering/window features for headless applications.
- [Bevy orthographic camera](https://bevy.org/examples/3d-rendering/orthographic/) and [glTF loading](https://bevy.org/examples/3d-rendering/load-gltf/) — V3 camera/model foundations; exact APIs remain pinned to the repository's Bevy 0.19.1 source.
- [Lightyear 0.29](https://docs.rs/lightyear/0.29.0/lightyear/) — version-pinned selected networking stack with server/client plugins, replication, input buffering, prediction, rollback, interpolation, and transport options.
- [Lightyear repository](https://github.com/cBournhonesque/lightyear) — supported Bevy versions, examples, transports, lag compensation, and project license.
- [Lightyear book Markdown source](https://github.com/cBournhonesque/lightyear/tree/main/book/src) — official guide to Lightyear concepts, setup, replication, prediction, rollback, and networking architecture.
- [Lightyear 0.29 network visibility example](https://github.com/cBournhonesque/lightyear/blob/0.29.0/examples/network_visibility/README.md) — version-pinned interest-management behavior and while-visible, retained, and always-present lifetimes.
- [Lightyear 0.29 immediate visibility source](https://github.com/cBournhonesque/lightyear/blob/0.29.0/crates/replication/replication/src/visibility/immediate.rs) and [room visibility source](https://github.com/cBournhonesque/lightyear/blob/0.29.0/crates/replication/replication/src/visibility/room.rs) — exact per-client visibility and semi-static room filtering APIs selected for future concealment research.
- [Avian](https://github.com/avianphysics/avian) — optional Bevy-native 2D collision and physics integration if custom queries are insufficient.
- [bevy_replicon](https://docs.rs/bevy_replicon/latest/bevy_replicon/) — modular server-authoritative replication fallback with pluggable transport.
- [Renet](https://docs.rs/renet/latest/renet/) — lower-level client/server transport option for the fallback stack.
- [Cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html) — Rust unit, integration, documentation, and benchmark test workflow.
- [rust-analyzer](https://rust-analyzer.github.io/manual.html) — Rust language-server tooling for navigation, completion, diagnostics, and refactoring.

## Historical alternatives and terrain references

The Godot material below informed the earlier engine comparison and the smooth
mask-to-visual-to-collision terrain alternative. These APIs are not implementation dependencies for
the selected Bevy stack or requirements for v1's quantized occupancy-grid design.

- [Godot license](https://godotengine.org/license/) — MIT licensing and treatment of engine versus game content.
- [Godot GitHub repository](https://github.com/godotengine/godot) — cross-platform 2D/3D engine overview and supported export targets.
- [Godot documentation](https://docs.godotengine.org/en/stable/) — official documentation for scenes, nodes, input, physics, and export workflows.
- [Godot BitMap](https://docs.godotengine.org/en/latest/classes/class_bitmap.html) — historical reference for binary masks and mask-to-polygon conversion.
- [Godot Geometry2D](https://docs.godotengine.org/en/stable/classes/class_geometry2d.html) — historical reference for polygon clipping, subtraction, and decomposition.
- [Godot ImageTexture](https://docs.godotengine.org/en/stable/classes/class_imagetexture.html) — historical reference for updating dynamic terrain textures.
- [Spell-Splosion](https://github.com/MitchMakesThings/Spell-Splosion) — older Godot 3 GDScript example demonstrating Worms-style terrain destruction. Its mask workflow is conceptually useful, but its engine APIs are not implementation guidance for Brawler.
- [Godot high-level multiplayer](https://docs.godotengine.org/en/4.0/tutorials/networking/high_level_multiplayer.html) — reference used during the engine comparison.
- [Godot dedicated server exports](https://docs.godotengine.org/en/stable/tutorials/export/exporting_for_dedicated_servers.html) — reference used during the engine comparison.

The [v1 Milestone 10 specification](./implementation/v1/milestone-10.md) records the exact local and
primary-source research used to select 8-unit cells, sparse 32×32-cell chunks, deterministic
half-cell brush quantization, Avian/Parry voxel collision, and per-chunk Bevy image updates across the
complete supported map-size range. Its primary exact-version references include
[Avian 0.7 collider documentation](https://docs.rs/avian2d/0.7.0/avian2d/collision/collider/struct.Collider.html),
[Parry 0.27 voxel source](https://docs.rs/parry2d/0.27.0/src/parry2d/shape/voxels/voxels.rs.html),
[Bevy 0.19 Image documentation](https://docs.rs/bevy/0.19.1/bevy/image/struct.Image.html), and
[Lightyear 0.29 source](https://github.com/cBournhonesque/lightyear/tree/0.29.0). Marching squares and
polygon simplification remain fallbacks only if measured voxel collision or gameplay evidence fails.

## Reference game and mode structure

- [Supercell game modes](https://support.supercell.com/brawl-stars/en/articles/game-modes-11.html) — current high-level descriptions for Showdown, Gem Grab, Wipeout, and related modes.
- [Supercell brawler classes](https://support.supercell.com/brawl-stars/en/articles/brawler-classes.html) — reference role taxonomy.
- [Supercell brawler traits](https://ingame.support.supercell.com/brawl-stars/en/articles/brawler-traits-5.html) — reference examples of alternate resource and passive rules.
- [Brawl Stars reference overview](https://www.noff.gg/brawl-stars/brawlers) — useful inventory of common fighter stats, roles, and trait patterns. This is an unofficial reference and should not be treated as authoritative balance data.
- [Hot Zone reference](https://brawlstars.fandom.com/wiki/Hot_Zone) — zone capture behavior and map implications. The page may be inaccessible in some environments, so use it as a secondary reference.
- [Brawl Stars overview](https://en.wikipedia.org/wiki/Brawl_Stars) — high-level summaries of objective modes, useful for cross-checking terminology; not a primary source.

## Research interpretation

The documents intentionally extract reusable primitives—health, movement, ammunition, projectiles, area control, pickups, objectives, respawn, and scoring—instead of copying named characters, exact values, art, or proprietary content.

The projectile specification deliberately separates trajectory, collision, gameplay payload, and presentation effects. This keeps second-phase behaviors such as bouncing, homing, curved paths, and richer particle effects compatible with the MVP's simpler projectile implementation.
