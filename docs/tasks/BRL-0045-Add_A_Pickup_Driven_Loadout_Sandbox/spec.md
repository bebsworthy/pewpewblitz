# Outcome

Provide a development-only, local Practice **Sandbox** mode whose core loop is moving through an arena and grabbing authored items that change the human player's ephemeral weapon and ultimate loadout. The mode is server-authoritative, immediately playable, repeatable, and isolated from saved-brawler persistence.

# Player loop

1. Enter Sandbox using an ordinary saved brawler as the starting fighter/profile/passive baseline.
2. Explore a dedicated arena containing clearly readable weapon-base, weapon-part, and ultimate item stations.
3. Move into range and press the existing Interact input. The server selects the nearest eligible item using stable ordering; proximity alone never causes an accidental swap.
4. The item applies at one fixed-tick boundary. The HUD and fighter presentation update from replicated authoritative state.
5. Shoot a target/bot, grab another item, and compare the result without leaving or reopening a menu.
6. Use a reset station or match-menu action to return to the admitted starting loadout and respawn all items/targets.

The first slice uses fixed, identifiable, replenishing stations rather than random loot. That makes the mode useful as a sandbox: the player can deliberately reach a known build instead of waiting for a random roll. Random chest drops can be a later game variant.

# Loadout-item rules

## Weapon-base item

- Replaces the current `WeaponBaseId`.
- Clears the four ephemeral part slots in the first slice. This avoids silently dropping an incompatible subset and makes the change legible.
- Resolves a complete new weapon through the production weapon catalog.
- Restores the new weapon to full/ready state.

## Weapon-part item

- References one authored `WeaponPartDefinitionId`; it never carries client-authored numeric effects.
- Appends to the first empty ephemeral slot.
- Rejects a duplicate definition and any aggregate that does not resolve through the production engine limits.
- When all four slots are full, replaces the oldest sandbox-acquired part (FIFO). The HUD previews `OLD -> NEW` while the player is in interaction range so replacement is not surprising.
- A dedicated clear-parts station empties all four slots without changing the weapon base.

## Ultimate item

- Replaces the current `UltimateDefinitionId`.
- Retires any deployable, cloak, concealment field, elemental field, targeted-aim arming, or other generation-owned runtime belonging to the previous ultimate.
- Starts the new ultimate at full charge in Sandbox so the player can test it immediately. This is a mode rule, not a change to ordinary match charge economy.

Fighter profile and the two passives remain those admitted with the saved brawler. They can become later pickup families only after the requested weapon/ultimate loop is proven.

# Authority and runtime model

```text
Authored sandbox item station
  stable item identity + grant (base / part / ultimate / clear / reset)
        |
player sends existing INTERACT input
        |
        v
Sandbox authority at fixed tick
  active match + owning fighter + proximity + readiness
  deterministic nearest-item selection
  resolve candidate IDs through active catalogs
  atomically replace ephemeral selection + resolved loadout + affected runtime
        |
        +-> replicate station cooldown/availability
        +-> replicate SandboxLoadoutState / SelectedBuild / ResolvedMatchLoadout
        +-> publish bounded collection cue and telemetry
```

The client sends no item ID, loadout shape, recipe fingerprint, or combat value. Interact is intent; the server decides what is in range and legal.

Add one bounded server-owned `SandboxLoadoutState` per participating fighter containing the starting selection, current weapon base, ordered zero-to-four part definition IDs, current ultimate, and revision. `ResolvedMatchLoadout` remains the combat/HUD truth. The explicit selection state is necessary because the resolved match loadout retains aggregate modifiers but not individual part identities or pickup order.

# In-place replacement semantics

Unlike the earlier editor/reset concept, grabbing an item should not restart the whole match. The game value is continuous pickup-driven experimentation.

- A newly accepted weapon affects attacks accepted after the transaction tick.
- Already committed projectiles/deliveries keep their copied old recipe and may finish normally; they cannot consult the new fighter loadout retroactively.
- Weapon replacement sets ammunition to the new capacity, clears fire/recovery deadlines, and clears buffered primary-fire input for the boundary tick.
- Ultimate replacement removes only the collector's prior ultimate-owned runtime and clears buffered ultimate/target-confirm input before installing full charge with `AbilityPhase::Ready`.
- Part/base/ultimate replacement updates `SelectedBuild` and a newly resolved `ResolvedMatchLoadout` atomically; rejection leaves every component unchanged and does not consume the station.
- Health, position, team, passives, score, target effects already applied to other entities, and opponent loadouts remain unchanged.
- Full sandbox reset uses the existing common match/map restart path to clear projectiles, deployables, fields, effects, pickups, objects, input buffers, trackers, and map generation coherently.

# Item lifecycle and map ownership

Do not overload `RestorationPickupDefinition` with loadout data. It is intentionally health-specific: collection requires missing health, facts/cues report restoration, and the existing chest terminal behavior names a restoration definition.

Add a focused Sandbox-owned item family, for example under `src/matchplay/sandbox/`:

- `SandboxItemDefinitionId` and `SandboxItemGrant` (`WeaponBase`, `WeaponPart`, `Ultimate`, `ClearParts`, `ResetLoadout`);
- replicated item identity, grant/display identity, position, and `ReadyAtTick`;
- bounded collection facts, cues, telemetry, and deterministic cooldown/replenishment;
- a pure candidate transition/resolution helper; and
- fixed-tick interaction/application systems with explicit ordering before combat input is accepted for the next tick.

The dedicated sandbox map owns bounded station anchors/placements. The mode plugin resolves those anchors into item entities after the selected map generation is ready. Items are visible durably through replicated entities; cues add collection/reappearance feedback but are not the source of truth.

Existing restoration pickup code supplies useful patterns for generation identity, capacity ceilings, stable collector ordering, replicated durable entities, cue deduplication, client reconciliation, bot views, and match-reset cleanup. Shared helpers should be extracted only where the second real use is exact; do not turn all pickups into a generic effect scripting framework.

# Mode topology

This concept now justifies a true mode boundary rather than a Wipeout capability flag:

- add `ModeDefinitionId(5)` and routed `GameMode::Sandbox`;
- add a local-only advertised Sandbox game type and Practice formation;
- add a dedicated sandbox map recipe with item anchors, targets, and valid spawns;
- install `SandboxModePlugin` beside Wipeout/Hot Zone/Heist while reusing common match lifecycle;
- provide no competitive score objective; use a long bounded session and explicit reset/leave paths;
- keep ordinary multiplayer catalogs free of Sandbox unless a later ticket deliberately makes it public; and
- keep bots from collecting loadout stations in the first slice. Existing bot/dummy combat behavior may remain, but only the human's Interact intent changes loadout.

Adding the routed enum value requires coordinated manifest/control codec, allocation policy, supervisor CLI, worker/config conversion, admission/lobby catalog, map-mode validation, HUD/diagnostics, process automation, and compatibility tests. Use the single global application compatibility handshake; do not add per-message versions.

# Presentation and UX

- Give base, part, ultimate, clear, and reset stations distinct silhouettes/colors plus readable world labels at interaction range.
- Display `INTERACT: Equip Heavy Payload`, `Swap Sentry -> Reveal Scan`, or `Replace oldest: Expanded Feed -> Quick Loader` from server-authored nearby-item facts.
- Extend the combat HUD with the current base, four ordered sandbox part slots, ultimate, and a short collection confirmation.
- Use existing catalog names and effects; do not duplicate display strings in the map recipe.
- Controller and keyboard use the existing Interact binding. Pickup collection must not require a pointer/editor overlay.
- Reconnect/late observation reconstructs current items and loadouts from durable replicated components, not transient cues.

# Balance Lab interaction

Sandbox resolves items against the active `BuildCatalog`, `WeaponCatalog`, and `WeaponPartCatalog`. A Balance Lab apply/restore must re-resolve each current sandbox selection against the new tuning snapshot atomically and keep its stable IDs/order when valid. If active tuning invalidates one selection, reject the Balance Lab transaction with the exact sandbox selection path instead of partially reverting the player.

Balance Lab's roster panel should identify the ephemeral sandbox override and show its stable base/ultimate plus effective part modifiers. It must not claim the parts are owned inventory instances.

# Acceptance criteria

1. A local player can enter the distinct Sandbox mode, move among item stations, and use Interact to equip weapon bases, zero-to-four parts, and ultimates without opening a loadout menu.
2. The server alone selects the nearest eligible station and resolves its stable-ID grant. Unknown, unavailable, out-of-range, duplicate, incompatible, over-capacity, non-sandbox, and non-active interactions do not mutate loadout or consume/cool down the item.
3. Base pickup clears parts; part pickup fills then FIFO-replaces with an explicit preview; clear-parts empties slots; ultimate pickup swaps and grants full Sandbox charge; reset restores the admitted selection.
4. `SandboxLoadoutState`, `SelectedBuild`, `ResolvedMatchLoadout`, weapon economy, ability runtime, and station readiness converge for the collector and late/reconnected observation.
5. Old committed attacks finish from their copied recipes; no new attack or old ultimate-owned runtime uses a mismatched loadout after the replacement boundary.
6. The saved brawler, inventory, equipped instances, revisions, canonical catalogs, opponent loadouts, and ordinary Practice/multiplayer flows remain unchanged.
7. Full reset and match restart restore station availability and clear all generation-owned runtime through existing lifecycle contracts.
8. Focused tests cover deterministic station choice, every grant transition, four-slot/FIFO behavior, duplicate/incompatible rejection, atomic rollback, owner-scoped ability cleanup, and reset.
9. Network/routed tests cover collection authority, replication convergence, repeated swaps, late observation/reconnect, profile non-mutation, mode admission, and capacity bounds. Native evidence covers readability, interaction prompts, HUD changes, and controller flow.
10. Documentation reconciles the fighter/loadout lifecycle, weapons/ultimates, networking, player UX, Balance Lab maintenance contract, maps, and the new mode.

# Scope exclusions

- arbitrary numeric weapon editing in-game;
- persistent loot, account inventory rewards, rarity, currencies, or progression;
- random procedural loot tables in the first slice;
- multiplayer competition or bots equipping items;
- fighter-profile/passive/active-item pickups;
- a generic executable pickup/effect scripting framework; and
- preserving ammo percentage, cooldown, ultimate charge, or active deployables across incompatible swaps.

# Estimated effort

A complete vertical slice is roughly 10-15 focused engineering days:

- 2-3 days: routed mode/catalog/map identity and sandbox item definitions/anchors;
- 3-4 days: authoritative item lifecycle, loadout transition/resolution, weapon/ultimate runtime policies;
- 2-3 days: durable replication, world presentation, prompts, HUD, audio/effects;
- 2-3 days: focused/network/routed verification and Balance Lab integration;
- 1-2 days: native iteration, feedback corrections, docs, and closeout.

The estimate is larger than an editor overlay because the pickup concept adds a real mode, authored arena content, durable world entities, interaction UX, and owner-scoped live replacement rules, but it produces a much stronger game loop.

# Dependencies and coordination

BRL-0003 changes elemental parts/ultimate fields and Balance Lab schemas; coordinate or implement after it so Sandbox exposes the accepted inventory and avoids duplicate compatibility migration. BRL-0033 may later teach bots to test or collect these builds, but it is not required for the first human-only loop.
