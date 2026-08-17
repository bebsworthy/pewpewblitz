# Engine decision

## Recommendation: Bevy 0.19 + Rust

Given Brawler's network-first architecture and the project's preference for Rust-first, code-driven, testable development, use **Bevy with Rust** rather than Godot/GDScript.

Bevy is a free, open-source Rust engine under dual MIT/Apache-2.0 licensing. It provides 2D/3D rendering, input, assets, UI, ECS scheduling, and modular plugins. Bevy 0.19 is the current baseline researched for this decision. [Bevy 0.19](https://bevy.org/news/bevy-0-19/) · [Bevy repository](https://github.com/bevyengine/bevy)

The networking baseline is **Lightyear**, not Bevy core. Lightyear currently provides Bevy-native client/server plugins, tick-buffered input networking, replication, client prediction, rollback, interpolation, interest management, lag compensation, and multiple transport options. [Lightyear repository](https://github.com/cBournhonesque/lightyear) · [Lightyear 0.29 documentation](https://docs.rs/lightyear/0.29.0/lightyear/)

Use **Avian 2D** for physics if the prototype needs a physics library beyond simple custom collision. Lightyear provides an Avian integration. [Avian](https://github.com/avianphysics/avian)

## Why this fits Brawler

- Rust is the primary development language; no separate GDScript language is required.
- Cargo, `rustc`, `rustfmt`, Clippy, rust-analyzer, and Rust's built-in test ecosystem provide a stronger general-purpose tooling baseline.
- ECS maps naturally to fighters, projectiles, effects, status meters, objectives, terrain chunks, and replicated entities.
- Headless server builds can omit rendering plugins while using the same Bevy application model.
- Gameplay and protocol-registration plugins can be tested in small Bevy `App`/`World` harnesses without opening a window.
- Lightyear's input, replication, prediction, and rollback features match the authoritative architecture already specified.
- A code-first workflow is a feature for this project, not a compromise.

## Important caveats

- Bevy has no first-party networking stack. Lightyear is a third-party dependency and should be pinned and upgraded deliberately.
- Bevy and Lightyear track each other's versions closely. A Bevy upgrade is a coordinated dependency migration, not a casual package update.
- Bevy has less mature authoring/editor infrastructure than Godot. This is acceptable because Brawler is intentionally code/data-driven, but map and asset authoring tools will be our responsibility.
- The eventual player map builder should edit a Brawler-owned serializable map recipe and preview
  it with Bevy; the Bevy `World` or editor UI must not become the only stored map representation.
- Rust compile times and ECS concepts add upfront complexity.
- Rust being compiled does not automatically make the game faster. The benefit is predictable control and efficient native execution; actual performance still depends on simulation, rendering, networking, and profiling.
- The Rust ecosystem is broader than the GDScript ecosystem, but the Bevy-specific ecosystem is smaller than Godot's. Prefer well-maintained dependencies that explicitly support the selected Bevy version, pin them, and keep their plugin/configuration usage localized enough to replace when evidence requires it.

## Networking stack decision

Prototype this stack first:

```text
Bevy 0.19
  + Lightyear 0.29
  + Avian 2D 0.7, if needed
  + evidence-based Cargo targets/features for a client and dedicated server
```

The first practical validation spans v1 Milestones 01–03: Bevy client/server application composition, a two-client connection/replication sandbox, and server-authoritative movement in a greybox arena. If Lightyear cannot meet the required behavior, integration, or maintenance standard at that gate, evaluate `bevy_replicon + Renet` as the modular fallback before combat content expands.

`bevy_replicon` provides server-authoritative replication but no I/O; it must be paired with a transport such as Renet, Renet2, or Quinnet. This is more flexible but leaves prediction and rollback more application-owned. [bevy_replicon](https://docs.rs/bevy_replicon/latest/bevy_replicon/) · [Renet](https://docs.rs/renet/latest/renet/)

## Alternatives considered

### Godot 4.x

Godot remains a strong general-purpose engine and is still a valid fallback. Its dedicated 2D workflow, editor, built-in multiplayer API, and asset pipeline provide a faster conventional start. The reasons it is not the current choice are project-specific: GDScript tooling and test organization are less attractive here, C# is intentionally excluded, and the team prefers a Rust/code-first workflow. [Godot license](https://godotengine.org/license/) · [Godot networking](https://docs.godotengine.org/en/stable/tutorials/networking/index.html)

### Defold

Defold is lightweight and strong for 2D deployment, but its workflow and ecosystem are less flexible for a game that may later blend 2D gameplay with 3D presentation or more complex authoring tools.

## Technical guardrails

- Use a fixed simulation tick for combat state updates.
- Keep gameplay definitions data-driven, but do not introduce a general-purpose ability scripting language yet.
- Keep rendering, input, and gameplay state separate enough that touch controls can be added later.
- Design the input layer around abstract actions and support an Xbox-like controller as the primary control scheme; keyboard/mouse is a supported parallel scheme for macOS development and play.
- Keep aim, fire, ability, interaction, and menu actions independent from physical button bindings so controller layouts can change without gameplay changes.
- Use collision layers and masks deliberately: fighters, projectiles, terrain, objectives, pickups, and hazards should not be one undifferentiated layer.
- Implement each mode as focused Bevy rule plugins, resources/components, and scheduled systems rather than branching victory logic through fighter or weapon systems. Do not require a universal mode trait before multiple modes demonstrate the need.
- Prefer serializable Rust definitions or authored Bevy assets/configuration files for content and focused Bevy systems for behavior.
- Keep gameplay definitions in serializable Rust data structures or authored data files; do not make the ECS world itself the only source of content definitions.
- Express the first built-in arena through the same bounded map-recipe schema and server resolver
  intended for later user-authored maps. Keep server-owned mode implementations separate from
  user-editable layout data.
- Keep destructible terrain as a quantized-solidity-to-visual-to-collision subsystem rather than
  encoding it as visible tile replacement. The authoritative occupancy grid is an internal gameplay
  representation and does not make presentation tiles authoritative.
- Queue terrain collision rebuilds between physics frames and rebuild only dirty terrain chunks.
- Treat the game as a dedicated-server-authoritative networked game from the first gameplay architecture.
- Clients send input commands and receive authoritative state/events; clients do not authoritatively submit positions, damage, hits, status changes, scores, or terrain changes.
- Keep client rendering, audio, camera, HUD, device input, and visual assets out of the dedicated-server build and plugin composition.
- Make offline and host-client testing exercise the same authoritative server systems and validation as a dedicated server; local convenience must not introduce a separate client-authoritative gameplay path.

## Bevy application composition

```text
Authored definitions / Bevy assets
              ↓
Authoritative gameplay World
  components, resources, systems,
  states, schedules, rule plugins
              ↕
Lightyear registration and networking
  inputs, replicated components, messages
              ↕
Client World and presentation
  replicated/predicted state, rendering,
  audio, camera, HUD, device input
```

Client and server entry points compose the appropriate Bevy base plugins, Brawler plugins, Cargo features, and process-level configuration. Runtime gameplay state lives in the authoritative server `World`; client worlds contain replicated or predicted copies plus local presentation state. A serializable gameplay component may also be a Lightyear-replicated component when that is the simplest correct representation—do not create a duplicate transport DTO solely to satisfy layering.

The diagram describes responsibilities, not folders or crates. Milestone 01 must compare single-package and small-workspace options using the actual Cargo feature graph. Another package, library target, or public API is justified only by a demonstrated platform, feature-isolation, compile-time, testing, or reuse boundary.
