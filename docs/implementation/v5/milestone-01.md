# V5 Milestone 01 — Auto-connect and player-dashboard vertical slice

Player-facing product name: **PewPew Blitz**. Historical repository/crate identifiers remain
unchanged during this version.

| Field | Value |
|---|---|
| Status | Complete |
| Depends on | V4 closeout accepted on 2026-08-21; completed V2 routed product flow and V3/V4 client presentation foundations |
| Outcome | Normal launch attempts one server and reaches a functional connected Player Dashboard whose real brawler, game-type, session, Play, Practice, and utility actions form the new product center |

## Research question

What is the smallest dashboard-centered vertical slice that removes the ordinary Title/Game Select
hub path, preserves bounded routed connection recovery and server authority, presents the player's
authored brawler as the visual focus, and remains fully operable by keyboard, gamepad, and pointer?

## Requested experience

The user supplied a Brawl Stars dashboard screenshot as a hierarchy reference: a large central
character, a selected-mode card below it, a dominant Play button, and secondary systems at the
edges. PewPew Blitz should adopt that hierarchy without copying its art or importing features the
game does not own.

The repository contains the initial dashboard concept at `inspiration/player_dashboard.png` and
the corrected lead concept at `inspiration/player_dashboard_2.png`:

![PewPew Blitz corrected player dashboard concept](../../../inspiration/player_dashboard_2.png)

This is the lead information-hierarchy and composition reference, not a production-asset contract.
The second concept removes the carousel, uses the current low-poly in-game presentation direction,
replaces the repeated bright wallpaper with a quiet navy field and localized cyan halo, and retains
the centered brawler, compact connected identity, bottom game-type card, adjacent Practice action,
dominant Play action, and quiet utility controls. Production uses the exact runtime character and
weapon presentation assets plus real authenticated/catalog data.

Concept details requiring correction or validation during specification/prototyping:

- remove the left/right brawler arrows and carousel behavior entirely; changing the brawler is one
  explicit activation into the build-selection surface;
- the generated fighter is mood/reference art only. The product dashboard renders the actual
  supported in-game player model, attached weapon, and idle animation for the current build, with
  the same primitive fallback policy as gameplay presentation;
- the avatar, hanger, skull, star, lightning, and logo treatments are concept iconography and should
  become original PewPew Blitz build/weapon/arena symbols rather than genre-reference echoes;
- replace the copied bright-blue genre wallpaper and repeated icons with a quiet original backdrop:
  a mostly static dark/cool field, one restrained slow-moving shader treatment, and a small soft
  glow localized behind the brawler;
- the mode card correctly describes a map pool rather than claiming that one map is selected;
- the player name, server name, budget, mode, rules, population, and map names must all resolve from
  current owners; the concept's literal sample values are not defaults;
- focus, disabled, stale-data, narrow-layout, and reduced-motion states are not represented in the
  still image and remain specification requirements.

The second concept resolves the large compositional issues. Remaining visual/product questions are
smaller and should not reopen its hierarchy:

- the pencil beside the build name and the `CHANGE BRAWLER` button currently duplicate the same
  apparent operation; retain one clear entry action unless renaming becomes a real separately owned
  feature;
- the hanger suggests cosmetics rather than authored build composition; use an original build,
  weapon, or wrench/node symbol;
- the skull/gamepad mode badge, trophy, player avatar, and crown remain generated concept marks and
  need an original PewPew Blitz icon pass; the visible `BRAWLER` wordmark is superseded by the
  accepted `inspiration/wordmark.png` direction in any subsequent concept;
- the white build/mode panels are readable but their final value, tint, and focus treatment should
  be evaluated against the actual model and dark background at supported UI scales;
- the halo is correctly localized and restrained; motion is intentionally absent from the still and
  should remain barely perceptible in the native prototype.

## Branding asset direction

The user accepted two repository concepts as the M01 branding sources:

| Source concept | Runtime role | Required preparation |
|---|---|---|
| `inspiration/logo2.png` | Connecting/loading full lockup | Preserve RGBA transparency; remove excess vertical canvas; add safe margin around the left streak/right impact; clean alpha edges; export a loading-sized master |
| `inspiration/wordmark.png` | Dashboard and compact shell headers | Preserve RGBA transparency; crop excess vertical canvas; remove isolated cyan/blue pixels; retain safe outline margin; verify at roughly 180–260 px display width |

M01 implementation owns this preparation; it is not an external asset blocker. Keep the source
concepts unchanged under `inspiration/`, derive cleaned runtime files into
`assets/brawler/ui/branding/`, and record that these are project-provided generated concepts rather
than third-party pack assets. Do not use a baked white/navy background in runtime variants.

The full lockup belongs on Connecting over the quiet procedural background. The wordmark belongs
in Dashboard and may be reused by Server Select, Settings, Credits, or Results when it remains
readable and does not displace screen-specific hierarchy. Small placements fall back to plain
styled `PEWPEW BLITZ` text if the outlined raster mark becomes visually noisy.

These carousel, actual-model, and simplified-background corrections were accepted by the user on
2026-08-21 during M01 research.

Initial PewPew Blitz translation:

```text
+------------------------------------------------------------------+
| PEWPEW BLITZ  accepted player  connected server          gear/menu|
|                                                                  |
|                         authored brawler                         |
|                    build name + compact identity                 |
|                         CHANGE BRAWLER                           |
|                                                                  |
| [ advertised game type / mode / map pool / population ] [PRACTICE] [PLAY] |
+------------------------------------------------------------------+
```

The whole brawler presentation and an explicit labeled control should open build selection. The
whole game-type card should open game selection. Pointer activation cannot be the only affordance;
focus, keyboard activation, and gamepad activation must expose the same actions.

## Current implementation findings

### Existing flow and reusable behavior

- `src/client/flow.rs` currently owns `Title`, `ServerSelect`, `Connecting`, `GameSelect`, `Queue`,
  `MatchLoading`, `Match`, and `Results`, plus build/error/confirmation overlays.
- `src/client/shell.rs` currently owns Title, Settings, Credits, persistent settings drafts,
  directional focus, pointer activation, entrance motion, UI scale, audio/display preferences, and
  match-menu Settings return.
- `src/client/server_select.rs` plus connection persistence already provide validated logical
  addresses, favorites, recents, and a configured product-server prefill.
- `src/client/flow.rs` already performs bounded DNS resolution, candidate attempts, cancellation,
  lobby handshake/catalog admission, recent-server persistence, structured failure actions, and
  cleanup between attempts.
- `src/client/queue.rs` already exposes authenticated game-type advertisements, population,
  formation availability, queue/practice requests, reservations, and loading phases.
- `src/client/build_editor.rs` and build persistence already own the last accepted local build,
  preset/custom selection, local preview, canonical validation, and save behavior.
- `src/client/presentation_3d/` owns the supported client-only 3D asset/model/animation path. A
  dashboard preview must share presentation profiles/assets where useful but have separate
  lifecycle ownership from replicated match fighters.
- The normal windowed product shell composes presentation state; explicit headless automation has
  an established bypass and must not acquire dashboard rendering.

### Product facts currently available to Dashboard

| Fact | Current owner/source | Default visibility candidate |
|---|---|---|
| Accepted display name | authenticated `ClientLobbyMembership` | Yes, compact header |
| Server display name | authenticated `ClientLobbyMembership` | Yes, compact header/status |
| Selected build source/name | build editor/persistence and embedded catalog | Yes |
| Build budget use | local resolved preview; revalidated on admission | Yes, compact |
| Weapon, ultimate, passives | local resolved build/catalog | Secondary detail, not all required at once |
| Game-type display name | lobby advertisement | Yes |
| Mode and team topology | lobby advertisement | Yes |
| Rules summary | lobby advertisement | Compact card/detail |
| Map pool | lobby advertisement plus embedded map display metadata | Yes; never imply one selected map |
| Waiting population | bounded lobby queue snapshot | Yes when fresh |
| Formation availability | bounded lobby queue snapshot | Yes as actionable availability state |
| Practice bot count | advertised team size minus the local player | Show near Practice when useful |
| Latency/ping | not currently measured as a product fact | No |
| Account level/currency/rank | no product owner | No |

## Local reference findings

The following checked-in references were inspected before external research:

- `references/bevy/examples/showcase/game_menu.rs` — independent top-level and menu-screen states,
  `OnEnter`, and state-scoped cleanup; supports keeping lifecycle transitions explicit instead of
  building an ad hoc screen stack.
- `references/bevy/examples/ui/navigation/directional_navigation.rs` and
  `directional_navigation_overrides.rs` — focus-visible spatial navigation for irregular dashboard
  layouts; the current Brawler shell already uses the compatible explicit map/focus pattern.
- `references/bevy/examples/ui/widgets/viewport_node.rs` — a UI-sized render target, dedicated 3D
  camera, `ViewportNode`, and picking. This is a viable focused-preview candidate, subject to exact
  Bevy 0.19 API verification and native cost/lifecycle testing.
- `references/bevy/examples/camera/first_person_view_model.rs` — separate render layers/cameras for
  view-owned presentation. Useful as an alternative if a viewport texture proves unnecessary or
  costly; not a reason to share the gameplay camera.
- `references/lightyear/book/src/tutorial/build_client_server.md` — connection lifecycle is
  represented by `Connecting`/`Connected`/`Disconnected` and explicit connect/disconnect triggers;
  V5 can change presentation entry without changing transport authority.
- `references/lightyear/book/src/concepts/connection/title.md` and
  `references/lightyear/book/src/guides/remote_server.md` — connection/authentication remains a
  network lifecycle concern independent from the dashboard.
- `/Users/boyd/.codex/skills/bevy-game-engine/references/assets-and-states.md` — retain asset handles
  and couple screen spawn/cleanup to explicit state transitions.

The checked-in Bevy source is 0.20-dev while Brawler uses 0.19. Exact APIs were therefore checked
against the installed `bevy-0.19.1`, `bevy_ui-0.19.1`, `bevy_ui_render-0.19.1`, and
`bevy_state-0.19.1` crate sources. Bevy 0.19.1 supports `ViewportNode`, an image render target,
`UiMaterialPlugin`/`MaterialNode`, per-camera transparent clear color, and `DespawnOnExit`.
`ViewportNode` does not own or remove its camera, so Dashboard must explicitly own the camera,
preview scene, dynamic image, animation graph/material handles, and their teardown. Existing Brawler
focus code remains the navigation baseline; no 0.20 automatic-navigation API is transferred.

## Current primary sources

- [Bevy Directional Navigation example](https://bevy.org/examples/ui-user-interface/directional-navigation/)
  — official spatial focus/navigation example for dynamic, irregular UI.
- [Bevy Directional Navigation Overrides example](https://bevy.org/examples/ui-user-interface/directional-navigation-overrides/)
  — official mixed automatic/manual navigation example for intentional focus routes.
- [Bevy Viewport Node example](https://bevy.org/examples/ui-user-interface/viewport-node/) — official
  dedicated 3D camera/render-target/UI viewport and picking example.
- [Bevy 0.19 `DespawnOnExit`](https://docs.rs/bevy/0.19.0/bevy/state/state_scoped/struct.DespawnOnExit.html)
  — exact state-owned entity cleanup contract used by the current client.

The local Lightyear 0.29 snapshot is newer and more applicable than the currently indexed public
docs observed during research, so exact Lightyear lifecycle guidance remains pinned to the local
book and checked-in production path rather than transferring a mismatched public version.

## Specification decisions

These decisions translate the accepted discussion direction into the M01 specification. Production
implementation remains gated on user acceptance of this document.

### Startup target and recovery

M01 defines “preferred” as the most recent successfully authenticated logical server already stored
at `recents[0]`. It does not add a second pin/preference field without a demonstrated need. Startup
chooses exactly one logical target in this order:

1. the explicit interactive `--server` value for this invocation;
2. the most recent successfully authenticated logical server;
3. the existing product default `127.0.0.1:5000`.

A successful manual selection naturally becomes the next launch target through the existing
bounded recent-server record. Favorites remain a Server Select convenience and do not silently
change startup. One logical DNS name may still resolve to at most the existing four address
candidates; that is one user-visible server, not multi-server failover.

Normal interactive launch enters Connecting and starts the existing bounded attempt. Cancel performs
orderly attempt cleanup and enters Server Select. Failure opens the existing structured error over
Server Select with `Retry` for the same logical address and `Choose Server` to dismiss the error.
Unexpected loss from Dashboard also leaves Dashboard immediately and exposes the same recovery
surface; stale authenticated facts are never displayed as connected.

An unreadable connections file does not supply a remembered target: startup uses the explicit or
default address and defers the existing local-data warning until the first stable Dashboard or
Server Select frame. Build/settings load failure likewise uses safe local defaults and reports the
real failure without blocking connection. No persistence schema changes in M01.

### Dashboard contents and actions

The default Dashboard uses the balanced information set:

- compact PewPew Blitz wordmark, accepted display name, authenticated server display name, and a
  connected indicator;
- the actual supported in-game character, attached current weapon presentation, and idle animation;
- build name (or `Custom Build`), used/available budget, and primary weapon name;
- advertised game-type name, mode, team size, concise rules, and map-pool display names;
- fresh waiting population and formation availability when the current snapshot matches the active
  catalog revision; otherwise explicit `Unavailable`/`Updating` copy rather than a retained number;
- `Change Brawler`, `Change Game`, dominant `Play`, secondary `Practice`, direct Settings, and a
  utility menu containing Credits, Change Server, and Quit.

No ping, rank, currency, progression, selected map, or account fact is synthesized. Long rules and
ultimate/passive detail stay in child surfaces. If Play or Practice is unavailable, the button is
disabled and carries the actual advertised/catalog reason. Activating the preview and activating
the labeled `Change Brawler` control are the same action; there is no carousel or second pencil
action.

Play submits the current locally resolved build and selected advertised game-type ID through the
existing authoritative queue request. Practice uses the same build and the existing authoritative
practice request. Neither action freezes, validates, allocates, selects a map, or creates a fighter
on the client. M01 may adapt the existing Build Editor and Game Select as temporary selection
surfaces; M02 owns their final dashboard-child presentation and all post-queue/match convergence.

### Preview and background choice

M01 selects a dedicated Bevy 0.19 `ViewportNode` with an image render target and dedicated
transparent-clear 3D camera. It gives responsive UI clipping/framing without sharing the gameplay
camera. The whole viewport sits inside an ordinary focusable/activatable UI control; M01 does not
add 3D mesh picking for a single action.

The preview resolves the same imported character scene, blaster scene, idle clip, scale/yaw
corrections, and primitive fallback used by gameplay presentation, but its entities are marked and
owned exclusively by the current Dashboard generation. It never creates `Fighter`, replicated
identity, combat state, physics, or network components. If imported assets are unavailable, the
current deterministic primitive character/weapon silhouette is shown instead of a generated or
static hero portrait.

The selected backdrop is one full-screen custom UI material behind the transparent viewport:

The lead direction is intentionally smaller than the concept wallpaper:

```text
near-solid cool background
       + very low-contrast, large-scale slow drift
       + one soft elliptical glow behind the brawler
       + grounded contact shadow/platform
```

The first shader candidate owns only normalized screen/preview UVs, elapsed presentation time, two
palette colors, glow center/radius, and motion strength. It should produce:

- a deep navy-to-muted-blue vertical gradient with no repeated icons or figurative pattern;
- one or two extremely broad bands/noise lobes drifting over roughly 20–30 seconds, with no obvious
  loop and no more than a few percent luminance change;
- a cyan/teal radial or elliptical glow centered behind the brawler's torso, feathered broadly and
  kept small enough that the outer dashboard stays calm;
- a nearly imperceptible 8–12 second glow “breath,” limited to a small opacity/radius change;
- no gameplay particles, screen distortion, rapid hue cycling, bloom dependency, or high-frequency
  noise.

`reduced_motion` freezes drift and breathing at a stable midpoint while retaining the static glow.
`reduced_combat_effects` does not need to control this shell treatment unless playtesting shows that
it is visually distracting. The effect remains client-only, bounded to Dashboard, and releases its
material/render-target ownership on exit.

The shader is client presentation only. Its material time advances only while Dashboard is active;
`reduced_motion` freezes drift and breathing at a stable midpoint. If shader creation or support is
unavailable, the dashboard retains a static navy background and static cyan glow using ordinary UI
nodes. No dashboard effect changes gameplay or navigation state.

### Branding preparation

Implementation derives two transparent runtime PNGs from the unchanged concept sources:

- `assets/brawler/ui/branding/pewpew-blitz-lockup.png` from `inspiration/logo2.png`;
- `assets/brawler/ui/branding/pewpew-blitz-wordmark.png` from `inspiration/wordmark.png`.

Both receive tight content cropping, deliberate safe padding, cleaned stray pixels/alpha edges, and
no baked background. The full lockup appears on Connecting; the wordmark appears in Dashboard.
These project-provided generated concepts are not added to the third-party CC0 asset manifest.
Focused asset checks verify shipped paths, nonzero dimensions, RGBA color type, and transparent
pixels; native checks verify real-size legibility and absence of seams/clipping. Player-facing
window title and shell copy become `PewPew Blitz`; crate names, paths, protocol identifiers,
environment variables, and existing persistence directories remain unchanged.

## Flow and focus contract

### M01 flow changes

`ClientFlow::Title` is retired from the ordinary product path and `ClientFlow::Dashboard` is added.
Connecting becomes the ordinary initial screen after startup resources are loaded.

| From | Trigger | To/result |
|---|---|---|
| Startup | one target resolved | Connecting; begin one bounded attempt |
| Connecting | authenticated welcome/catalog accepted | Dashboard |
| Connecting | Cancel | orderly cleanup, then Server Select |
| Connecting | bounded failure | Server Select plus Retry/Choose Server error |
| Server Select | valid Connect | Connecting |
| Dashboard | Change Brawler/preview activation | existing Build Editor selection surface |
| Dashboard | Change Game/game card activation | existing Game Select selection surface |
| Dashboard | Play accepted | Queue |
| Dashboard | Practice accepted | Match Loading |
| Dashboard | Settings/Credits | existing overlay; close returns focus to invoker |
| Dashboard | Change Server | disconnect confirmation, orderly cleanup, Server Select |
| Dashboard | unexpected disconnect | Server Select plus classified recovery error |

M01 does not claim the final connected return loop: queue cancellation, loading cancellation,
confirmed match leave, and Results convergence remain explicitly owned by M02. Until then those
paths retain their accepted V2 behavior and are a documented playtest limitation, not a second
Dashboard implementation.

### Focus and input

The existing explicit directional focus graph remains authoritative across keyboard and gamepad;
pointer activation synchronizes focus before action. Initial Dashboard focus is Play. The main row
orders Game Type -> Practice -> Play; up reaches Change Brawler/preview; utilities are reachable
without cycling through decorative content. Escape/East closes an overlay to its exact invoker and
does not disconnect. Connecting initial focus is Cancel. Server Select retains its established
editable-field and Settings access. Decorative model, logo, glow, and status text are not focusable.

## ECS ownership and lifecycle

### State and resources

- `ClientFlow` remains the sole top-level product navigation state. `ClientOverlay` remains the sole
  modal owner; no screen stack or second reducer is introduced.
- A small client-only `DashboardViewModel` is rebuilt from `ClientLobbyMembership`, the selected
  advertisement/snapshot, embedded catalogs, and `BuildEditorState::loaded_selection`. It contains
  display-ready real facts and explicit freshness/availability states, not authority.
- `DashboardPreviewGeneration` identifies the currently owned viewport, camera, render target,
  preview root, scene/weapon roots, animation player linkage, and fallback entities. A generation
  guards late asynchronous asset/scene callbacks after Dashboard exit or re-entry.
- Dashboard shader material/elapsed presentation time is client-only. Reduced-motion settings are
  read from the existing persisted settings resource.
- Existing `PendingFlowActions`/`FlowCommit` remain the single transition arbitration path.
  Dashboard buttons feed that path; render completion and animation never drive navigation.

### Composition and schedule

The client gains a cohesive private dashboard module, split only where ownership differs:

```text
src/client/dashboard/
  mod.rs          plugin, view model, UI composition, actions, tests
  preview.rs      viewport/camera/model/animation/fallback lifecycle
  background.rs   custom UI material and reduced-motion uniforms
assets/brawler/shaders/dashboard_background.wgsl
```

Shared imported fighter asset/profile preparation moves only as far as needed for the existing
gameplay renderer and Dashboard preview to consume the same corrections. It does not become a new
public crate/API.

Startup ordering is explicit: load connection/build/settings state, apply deferred resource
commands, resolve the single target, then allow the initial Connecting entry to begin the attempt.
The existing chained flow phases remain:

```text
BeginFlowFrame -> ObserveSession -> CollectFlowInput -> ResolveFlowAction
 -> TeardownSession -> CommitFlow -> PresentFlow
```

Dashboard input is collected in `CollectFlowInput`; state changes occur only in `CommitFlow`.
Dashboard screen, preview, and background entities use state-scoped ownership plus explicit cleanup
for non-entity assets. Preview scene/animation binding runs only for the active generation and only
after required handles are ready. Dynamic render-target resize follows the computed viewport size
and is clamped to a bounded nonzero size; resize never recreates networking or gameplay state.

On Dashboard exit, the preview camera, scene roots, fallback entities, viewport nodes, and background
entities despawn. Dynamic image/material handles and generation state are removed so repeated
Dashboard entries do not accumulate render targets, cameras, animation graphs, or materials.

## Network and authority behavior

M01 changes no protocol or server behavior. It reuses the current routed lobby connection,
compatibility handshake, catalog admission, queue/practice requests, reservations, match transfer,
and disconnect teardown. The Dashboard exists only with a valid authenticated lobby membership.
Clients continue to send display-name, connection, selected-build, queue, practice, cancel, and
disconnect intent; the server continues to own identity acceptance, catalog/game-type truth, build
legality at admission, population, formation, map choice, worker allocation, gameplay, and outcome.

Normal interactive product clients use the flow-owned auto-connect path. Existing explicit
`--auto-connect`, headless, controller/combat demos, screenshot/report automation, and routed E2E
paths retain their established presentation bypass and numeric-socket requirements. The dedicated
server feature graph does not acquire UI, rendering, image, animation, input-device, or dashboard
assets.

## Risks and constraints

1. **Network-gated home:** Dashboard requires valid lobby facts. Settings must remain reachable when
   auto-connect fails so input/accessibility configuration is never network-gated.
2. **Terminology:** the center represents an authored brawler/build, not a roster hero and not the
   server-owned runtime fighter.
3. **Preview lifecycle:** a second camera, render target, scene graph, animation player, weapon
   attachment, and asset handles must have explicit dashboard-generation ownership and teardown.
4. **Responsive focus:** a central interactive viewport plus bottom cards and utilities creates an
   irregular focus graph. Initial focus, back behavior, pointer-to-focus synchronization, and
   controller routes must be specified, not left to incidental entity order.
5. **Stale data:** population and availability need an honest unavailable/stale state rather than
   preserving the last number indefinitely.
6. **Selection versus admission:** editing the dashboard draft must not mutate a queued/accepted
   build. Play/Practice still submits bounded intent for authoritative validation.
7. **Recovery:** unexpected loss cannot return to Dashboard because its facts are no longer valid.
   The recovery path must say whether it retries the same server or exposes Server Select.
8. **Scope pressure:** the reference screenshot contains progression/social surfaces. Empty space is
   preferable to inventing placeholder systems.

## Implementation plan

1. Retire ordinary Title entry, add Dashboard, resolve the deterministic startup target from the
   existing CLI/recent/default owners, and route success/cancel/failure/unexpected loss according to
   the transition table.
2. Add the Dashboard view model and UI using existing shell controls, focus, overlays, settings,
   build/game-type owners, and one flow-action arbitration path.
3. Extract the minimal shared fighter-presentation profile/asset preparation, implement the
   dashboard viewport, actual character/weapon/idle path, primitive fallback, resize, generation,
   and teardown.
4. Implement the bounded background UI material, reduced-motion/static fallback, and localized
   model grounding/glow.
5. Clean and promote the two accepted branding concepts, update player-facing PewPew Blitz window
   and shell copy, retain historical internal identifiers, and add focused asset validation.
6. Connect Play, Practice, temporary selection entries, Settings, Credits, Change Server, Quit, and
   disabled/stale states without changing authority or M02 return-loop scope.
7. Run focused tests and canonical role checks, perform the native visual/input/lifecycle matrix,
   then hand the build to the user with M02-owned limitations stated explicitly.

## First playtest feedback review — 2026-08-21

The first native comparison confirmed that the information and authority contracts are correct but
that the dashboard still reads as a functional wireframe beside the accepted visual target.

| Feedback | Decision |
|---|---|
| The opaque preview reads as a black screen inside the dashboard | Implement now: visually merge its clear color with the procedural field and reduce the apparent viewport boundary |
| The actual fighter is too small/low and its weapon obscures the face and torso | Implement now: enlarge and raise the framing, use a three-quarter presentation angle, and retune key/rim lighting |
| The fighter lacks the target's halo and grounding | Implement now: retain the procedural halo and add a restrained client-only platform/contact treatment using existing primitives |
| Build and game facts read as debug text rather than deliberate cards | Implement now: introduce bounded card containers, stronger type hierarchy, borders, and left-aligned fact grouping while preserving real data |
| Play is prominent by size but not by visual energy | Implement now: use the accepted warm primary color, stronger outline, and larger label; keep Practice visibly secondary |
| The bottom utility row is visually heavy | Implement now: move Settings and a compact Menu action into the header; the menu owns Credits, Change Server, and Quit without inventing new product features |
| The target uses generated fighter art, obsolete branding, and invented values | Retain the implementation: actual runtime fighter, PewPew Blitz wordmark, and authenticated/catalog facts remain required |
| A final icon/display-font art pass would improve finish | Defer to M03: do not block this hierarchy correction on new external assets or introduce placeholder genre icons |
| The fighter preview becomes a dark rectangle when hovered | Implement now: keep the preview fill transparent for rest, focus, hover, and press; use only a cyan border as its interaction cue |

This feedback is accepted M01 scope because it evaluates the milestone's explicit native hierarchy,
model framing, background integration, and Play-versus-Practice exit criteria. Queue/results
convergence and final child-screen redesign remain M02.

## Second visual feedback execution plan — 2026-08-21

The second target comparison accepted a further M01 presentation pass. The existing hierarchy and
real-data contract remain correct; the remaining gap is the dashboard's visual language and central
composition.

1. Promote the unmodified `external_assets/Lilita_One/LilitaOne-Regular.ttf` under SIL OFL 1.1,
   retain it through the client asset owner, ship its license, and use it for dashboard display
   headings and actions. Keep compact live values in Fira Mono rather than forcing a display face
   into dense status copy.
2. Promote one coherent CC0 Kenney game-icon subset for Settings, Menu, build, mode, Practice, and
   Play. `kenney_ui-pack` was requested as a preferred source but is not present in the workspace at
   implementation start, so the checked-in `kenney_game-icons` pack is the available coherent
   source; no generated genre-reference icons are introduced.
3. Raise and enlarge the actual runtime fighter, retune its camera/pose/light for a clear face and
   silhouette, tighten the localized halo, and replace the oversized flat platform with a smaller
   layered pedestal.
4. Narrow and visually attach the build card to the fighter stack. Add icon wells, layered borders,
   stronger Lilita One headings, and compact real-data rows to the mode, Practice, and Play actions.
5. Preserve explicit rest, pointer-hover, focus, press, and disabled states. The fighter-preview
   fill remains transparent in every state; only its outline may change.
6. Verify the native 16:9 composition at 1280x720 and 1920x1080, long real labels, pointer/keyboard/
   gamepad focus, imported and primitive fighter paths, asset failure fallback, and canonical role
   isolation before returning M01 to User playtest.

## Verification plan

### Focused automated coverage

- startup precedence: explicit interactive address, most recent success, product default, corrupt
  persistence, retry same target, and cancel cleanup;
- flow transitions: connecting success/failure/cancel, Dashboard action routing, disconnect
  confirmation, unexpected loss, and overlay focus restoration;
- Dashboard view model: real build/catalog/member facts, map-pool formatting, fresh/stale population,
  unsupported Play/Practice reasons, and no selected-map or invented-fact fallback;
- preview lifecycle: one active camera/render target/generation, late-scene rejection after exit,
  imported/fallback paths, bounded resize, and no accumulation across repeated entries;
- reduced-motion shader state and static fallback selection;
- branding file path, PNG/RGBA dimensions, transparency, and retained source concepts;
- existing connection persistence round trips and recent ordering remain compatible;
- client composition includes Dashboard only in the interactive shell; server composition remains
  free of client presentation dependencies.

### Canonical automated gate

Run `just fmt`, `just lint`, `just check`, and `just test`. Run `just e2e 2` after affected routed
tests pass to prove the unchanged lobby/admission/match path. Broader 4/6-client closeout remains
M03 unless M01 changes expose a routing regression.

### Native visual and interaction matrix

- normal first launch, remembered-success launch, explicit `--server`, bounded failure, Retry,
  Choose Server, Cancel, Change Server, and unexpected loss;
- 1280x720 and 1920x1080 at default scale, plus the existing minimum supported window/UI-scale
  extremes; verify clipping, model framing, long server/build/game-type names, and map-pool wrapping;
- keyboard, gamepad, and pointer parity, including initial focus, viewport activation, utility menu,
  overlay close, and pointer-to-focus synchronization;
- imported model/weapon/idle, forced primitive fallback, slow background over at least 30 seconds,
  reduced motion, static shader fallback, repeated Dashboard entry/exit, and no model/logo seam;
- logo lockup on Connecting and compact wordmark at actual header width with transparent edges;
- Dashboard frame behavior compared with the existing shell, recording preview target size and
  camera/material counts so a lifecycle or obvious frame-cost regression cannot be accepted by eye.

## User playtest handoff

Use the canonical `just server` and `just client` path. Evaluate launch/recovery clarity, whether the
actual fighter remains the visual center, information density, Play-versus-Practice hierarchy,
model/background motion, logo legibility, and keyboard/gamepad/pointer navigation. Queue, loading,
match-leave, and Results convergence plus final child selection surfaces remain M02 work.

## Implementation and verification evidence

Implemented on 2026-08-21:

- ordinary interactive startup now enters Connecting and chooses one target in the accepted
  explicit-address, most-recent-success, local-default order;
- accepted lobby membership enters Dashboard, while cancel, bounded failure, orderly Change Server,
  and unexpected loss use the Server Select recovery surface;
- Dashboard presents the authenticated player/server, current persisted build and budget, weapon,
  advertised game type/rules/map pool, fresh population/availability, actual imported fighter,
  attached weapon, idle animation, and primitive fallback;
- Play and Practice reuse the existing authoritative lobby requests; build/game selection reuse the
  current temporary child surfaces; Settings, Credits, Change Server confirmation, and Quit are
  directly reachable;
- `src/client/dashboard.rs` owns the cohesive viewport/background lifecycle. A separate subdirectory
  was not justified by the implemented size: the camera, dynamic image, scene hierarchy, custom UI
  material, and state-scoped cleanup remain easy to audit together;
- the background uses `assets/brawler/shaders/dashboard_background.wgsl`, freezes with reduced
  motion, has a static navy fallback, and uses a client-only cyan rim light on the preview model;
- exact transparent crops of the user-provided logo concepts ship as the loading lockup and compact
  wordmark. Image-generation cleanup attempts were rejected because they baked visible backgrounds;
  the unchanged RGBA source pixels were retained instead;
- player-facing window, credits, and local server names use PewPew Blitz; internal crate, protocol,
  environment, and persistence identities remain unchanged.

First-feedback refinement implemented on 2026-08-21:

- the preview now merges into the procedural field with a transparent camera clear and transparent
  UI fill in every interaction state; fighter activation uses a cyan hover/focus outline without
  covering the model or halo;
- the actual fighter is larger and raised, uses a three-quarter holding pose with improved key/rim
  lighting, and stands on a restrained platform inside the localized procedural glow;
- the build and game facts now use bright bounded cards with stronger hierarchy, Practice is a
  secondary pale card, and Play has an orange resting background plus a brighter warm hover state;
- Settings and Menu moved into the header; Menu owns Credits, Change Server, Quit, and Back without
  adding product features or leaving an authenticated Dashboard session.

Second visual-feedback refinement implemented on 2026-08-21:

- the unchanged Lilita One regular font is promoted as the dashboard display face, retained by the
  client asset owner, manifested as SIL OFL 1.1, attributed in Credits, and accompanied by its full
  shipped license; compact server, population, rules, map-pool, weapon, and budget facts remain in
  Fira Mono for small-size readability;
- six white CC0 Kenney Game Icons now provide one coherent Settings, Menu, build, mode, Practice,
  and Play family. The requested `kenney_ui-pack` was not present in the workspace, so no untracked
  or invented substitute source was assumed;
- the Dashboard uses Lilita One headings and actions, blue icon wells, bounded drop shadows, a warm
  white-on-orange Play treatment, a pale secondary Practice treatment, and explicit distinct hover
  fills for every actionable card family;
- the actual fighter is larger and more readable with a reduced dashboard-only weapon scale, warm
  frontal light, cyan rim light, tighter shader halo, and a smaller two-layer blue pedestal;
- the fighter/build stack now has cohesive central ownership: it remains centered between the fixed
  header and bottom action row, caps its preview height on taller windows, and keeps the build card
  attached without hiding or clipping it;
- the whole fighter preview remains transparent in rest, hover, focus, and press states. It retains
  only its cyan outline interaction cue.

Automated evidence:

- `just lint` passed, including client/server Clippy, server feature isolation, and the retired 2D
  renderer guard;
- `just check` passed for routing, client, server, and network-test feature graphs;
- `just test` passed: 83 routing library tests plus routing process suites, 396 client tests, 311
  server tests, 82 serialized network integration tests, and 14 performance gates;
- `just e2e 2` passed with one exact routed 1v1 roster reaching Active and both workers shutting down
  cleanly;
- native 1280x720 captures verified the transparent wordmark, authenticated Dashboard facts, actual
  imported fighter/weapon/idle path, dominant Play hierarchy, shader compilation, and bounded fit.
  The initial composition capture is `target/v5-m01-screens-final2/brawler-000360.png`; the
  first-feedback composition capture is
  `target/v5-m01-feedback-final-transparent/brawler-000240.png`. Automated capture occasionally records partially
  uploaded glyph atlases, so live text/focus rendering remains part of the user playtest rather than
  being accepted from that frame alone.
- after the first-feedback refinement, `just lint` and `just check` passed; all 396 existing client
  library tests passed, and the added focused preview-interaction test proves that rest, hover, and
  press retain a transparent fighter-preview fill while hover retains a visible border cue.
- after the second visual-feedback refinement, `just lint` and `just check` passed, including the
  independent client/server feature graphs and server presentation-isolation guards; all 399 client
  library tests passed. New focused coverage validates promoted font/icon/license presence and
  manifest membership plus visibly distinct header/card/Practice/Play hover colors.
- native comparison captures at the exact 1280x720 logical request and the host-constrained larger
  window verified the Lilita One hierarchy, coherent icons, real-data wrapping, model lighting,
  pedestal, bottom-row anchoring, and absence of clipping. The representative captures are
  `target/v5-m01-ui-responsive-1280/brawler-000180.png` and
  `target/v5-m01-ui-responsive-1920/brawler-000180.png`; macOS constrained the requested 1920x1080
  window to the available display work area, producing a 3420x2082 Retina framebuffer. Exact
  1920x1080 hardware verification remains part of the user/M03 matrix rather than being inferred.

## Feedback review and acceptance

Closed on 2026-08-21 after the user accepted the refined dashboard by directing that M01 be
committed, closed, and followed by M02. The accepted pass includes the centered actual fighter,
transparent fighter activation surface, restrained procedural field and glow, attached real-data
cards, Lilita One display hierarchy, coherent Kenney icon family, dominant Play action, secondary
Practice action, and compact authenticated header.

Feedback dispositions:

- the dark fighter-preview hover regression was fixed in M01 and is protected by focused tests;
- the requested Lilita One face and available Kenney Game Icons were promoted with manifests,
  attribution, and shipped licenses;
- `kenney_ui-pack` was not present in `external_assets`; the available coherent CC0
  `kenney_game-icons` family was accepted for this pass rather than inventing an asset source;
- exact 1920x1080 hardware coverage and the exhaustive resolution, controller, repeated-entry, and
  lifecycle matrix remain explicitly assigned to M03. The host-constrained high-resolution Retina
  capture is supporting evidence, not a substitute for that closeout matrix;
- dashboard-child selection and connected match-loop convergence remain M02 scope.

## Learn-from-errors review

- The first functional dashboard treated most information and actions as uniform monospace blocks.
  That weakened scan order even though the data was correct. Future shell work should establish
  display/body typography roles and a coherent icon family before fine spacing work.
- The fighter activation surface initially inherited the generic button hover fill, obscuring the
  model with a dark rectangle. Preview interaction now owns an explicit transparent-fill invariant
  for every state, with a regression test; distinctive focus belongs on the outline.
- Direct root-level percentage sizing produced excessive empty space on taller windows. The final
  layout gives the fighter and build card one cohesive center wrapper and anchors the action row,
  which is the reusable pattern for M02 children.
- A late negative-margin experiment could hide the build card behind the viewport. It was reverted
  before delivery. Any future overlap or stacking change must be checked in a native capture as
  well as through layout tests.
- Client-retained font and image handles, explicit asset manifests/licenses, and dashboard-only
  camera/light/model scaling delivered the visual change without leaking presentation ownership
  into gameplay or server feature graphs. Preserve that boundary in later dashboard screens.

M01 is complete. The intentionally deferred M02/M03 work below remains visible and does not weaken
the accepted vertical-slice result.

## Exit criteria

M01 completion required:

- ordinary launch never visits Title and deterministically attempts one visible logical server;
- cancellation and bounded failure reach a fully usable Server Select, while success reaches a
  valid authenticated Dashboard;
- Dashboard displays only the specified real facts and all required actions function with keyboard,
  gamepad, and pointer;
- the actual supported character, weapon, idle animation, and primitive fallback render with no
  gameplay/replication ownership and clean up across repeated entries;
- the restrained shader/glow, reduced-motion state, and static fallback are readable and do not
  control flow;
- cleaned transparent PewPew Blitz lockup/wordmark assets work at their real runtime sizes and all
  player-facing shell branding is updated without an internal compatibility rename;
- focused tests, canonical checks/tests, routed two-client E2E, and the native matrix pass;
- M02-owned return-loop limitations remain explicit, user feedback is triaged, affected checks are
  rerun, and the milestone learning review is recorded before status becomes Complete.

## Explicitly deferred to M02/M03

- final Dashboard-child Build and Game selection presentation and confirm/back semantics;
- queue/loading/leave/results convergence and exact Play Again behavior;
- exhaustive responsive, accessibility, transition/audio, lifecycle/performance hardening and the
  2/4/6-client V5 closeout matrix;
- original final icon/art pass beyond the accepted cleaned branding and procedural Dashboard field.
