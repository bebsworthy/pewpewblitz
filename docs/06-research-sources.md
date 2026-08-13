# Research sources

The following sources informed the initial design baseline. They describe the reference genre and engine capabilities; Brawler should use original content and terminology.

## Engine

- [Godot license](https://godotengine.org/license/) — MIT licensing and treatment of engine versus game content.
- [Godot GitHub repository](https://github.com/godotengine/godot) — cross-platform 2D/3D engine overview and supported export targets.
- [Godot documentation](https://docs.godotengine.org/en/stable/) — official documentation for scenes, nodes, input, physics, and export workflows.
- [Godot BitMap](https://docs.godotengine.org/en/latest/classes/class_bitmap.html) — binary masks and mask-to-polygon conversion through `opaque_to_polygons()`.
- [Godot Geometry2D](https://docs.godotengine.org/en/stable/classes/class_geometry2d.html) — polygon clipping, subtraction, and decomposition helpers.
- [Godot ImageTexture](https://docs.godotengine.org/en/stable/classes/class_imagetexture.html) — updating dynamic terrain textures efficiently.
- [Spell-Splosion](https://github.com/MitchMakesThings/Spell-Splosion) — older Godot 3 GDScript example project demonstrating Worms-style 2D terrain destruction. The repository code is MIT-licensed; its README identifies Kenney and project-created assets separately, so asset licenses must be checked independently.
- [Godot high-level multiplayer](https://docs.godotengine.org/en/4.0/tutorials/networking/high_level_multiplayer.html) — Godot's server/client networking model and high-level multiplayer API.
- [Godot dedicated server exports](https://docs.godotengine.org/en/stable/tutorials/export/exporting_for_dedicated_servers.html) — headless and dedicated-server export workflow.
- [Bevy 0.19](https://bevy.org/news/bevy-0-19/) — current release baseline, code-first scenes, ECS, and engine development status.
- [Bevy ECS guide](https://bevy.org/learn/quick-start/getting-started/ecs/) — entities, components, systems, queries, and resources.
- [Bevy plugins and headless servers](https://bevy.org/learn/quick-start/getting-started/plugins/) — modular plugin model and omission of rendering plugins for headless applications.
- [Lightyear](https://docs.rs/lightyear/latest/lightyear/) — Bevy networking stack with server/client plugins, replication, input buffering, prediction, rollback, and interpolation.
- [Lightyear repository](https://github.com/cBournhonesque/lightyear) — supported Bevy versions, transports, lag compensation, and project license.
- [bevy_replicon](https://docs.rs/bevy_replicon/latest/bevy_replicon/) — modular server-authoritative replication alternative with pluggable transport.
- [Renet](https://docs.rs/renet/latest/renet/) — lower-level client/server transport, authentication, encryption, and message channels.
- [Cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html) — Rust unit, integration, documentation, and benchmark test workflow.
- [rust-analyzer](https://rust-analyzer.github.io/manual.html) — Rust language-server tooling for navigation, completion, diagnostics, and refactoring.

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
