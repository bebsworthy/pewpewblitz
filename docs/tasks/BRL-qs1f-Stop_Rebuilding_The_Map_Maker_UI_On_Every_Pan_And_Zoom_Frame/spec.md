# Outcome

Panning and zooming in the Map Maker is cheap and smooth.

# Context

- Middle-drag pan calls `runtime.bump_render()` on every frame with mouse motion (`src/client/flow/screens/map_maker.rs:1428-1430`). Wheel zoom does the same (`:1444`).
- `present_map_editor` despawns and respawns all editor UI roots whenever `render_revision` changes (`:2006-2014`).
- `bump_render` has about 51 call sites.

# Scope

- Camera pan and zoom update the editor camera only.
- Audit the other `bump_render` callers for per-frame triggers.
- Retain UI nodes where state only changes values.

# Out of scope

Map Maker feature work (BRL-6vjz, BRL-tztp).

# Acceptance criteria

- Five seconds of continuous panning or zooming produces zero editor UI root respawns (ECS test).
- Visible behavior is unchanged.

# Verification

- The ECS test.
- `just test`.
- Native check.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Research and design (2026-11-12)

- Camera flow: `map_maker_input` writes `MapMakerRuntime.camera_center/camera_zoom` → `sync_map_editor_preview` copies them into `MapEditorPreview` → the 3D camera system positions `ArenaCamera`. None of that needs the editor UI to be rebuilt.
- The only editor UI that depends on the camera is the selection toolbar, whose screen position comes from `world_to_viewport` of the selected cells. Today it follows pan/zoom only because the whole UI is respawned each frame (and it already does not follow Fit View, which never bumped the render revision).
- `bump_render` audit (~51 sites): every other caller is one-shot — a resolved `FlowUiAction`, a key/button `just_pressed`, a search-text edit that actually changed text, a mouse-wheel step over the palette (discrete action), stroke commit on release, or the Practice return. Pointer-cursor movement and in-progress strokes already avoid it (cursor copy is a retained `Text` update; stroke preview is the 3D `MapEditorPreview`). Only middle/space-drag pan and wheel zoom bump per frame.

Decision (smallest clear change):
1. Pan and wheel zoom stop calling `bump_render`; they only change the runtime camera fields (the preview/camera path is unchanged).
2. Mark the selection toolbar node with a private `MapMakerSelectionToolbar` component and add `position_map_editor_selection_toolbar` (PresentFlow, after `present_map_editor`) that updates only that node's `left/top` when the runtime or the editor camera transform changed. Spawn and reposition share one offset helper so placement is identical. Side effect: the toolbar now also follows Fit View.
3. No general retained-UI conversion of the discrete-action rebuilds in this ticket; those rebuild once per user action, not per frame. Any broader retained-mode work goes to a separate backlog ticket.

Test: ECS test in `map_maker.rs` drives 5 s of frames (`SIMULATION_TICK` manual time steps, no sleeps) with middle-button pan motion and wheel zoom every frame, asserts the camera fields changed, and that the single `MapMakerRoot` entity is never despawned/respawned.


# Implementation (2026-11-12)

- `src/client/flow/screens/map_maker.rs`: middle/space-drag pan and wheel zoom no longer call `bump_render`; they only update `camera_center`/`camera_zoom` (consumed unchanged by `sync_map_editor_preview` → editor camera). The selection toolbar is tagged `MapMakerSelectionToolbar`; new `position_map_editor_selection_toolbar` updates only its `Node.left/top` when the runtime or the `ArenaCamera` transform changed, using the same `selection_toolbar_offset` helper as the spawn path. New ECS test `five_seconds_of_pan_and_zoom_never_respawn_the_editor_ui`.
- `src/client/flow.rs`: registers the new system in `ClientFlowSet::PresentFlow`, after `present_map_editor`, gated to `ClientFlow::MapEditor` (map-editor present systems grouped in a nested tuple to stay under Bevy's tuple limit).
- Other 49 `bump_render` callers left as-is: all one-shot per user action (audit above).

# Verification (2026-11-12)

- `cargo test --locked --no-default-features --features client --lib five_seconds_of_pan` — pass. 300 frames (`simulation_ticks_from_seconds(5)`, manual `SIMULATION_TICK` time steps, no sleeps) of middle-drag motion plus a wheel event every frame, with a selection active: the single `MapMakerRoot` and `MapMakerSelectionToolbar` entities are identical every frame; camera center and zoom changed.
- Negative check: temporarily restoring the zoom `bump_render` makes the test fail at frame 0 ("respawned editor UI"); reverted.
- `just lint` (real ripgrep 14.1.1 first on PATH so the scripted checks run) — pass, including client/server/network/balance-lab clippy `-D warnings`, server-feature, V3, and V8 checks.
- `just test` (same PATH) — pass; all suites ok, 0 failures, including `client_flow_update_schedule_has_no_owned_ambiguities`.
- Native check: not run — this session cannot drive the macOS client window interactively. Pending a playtest.

# Playtest request

Run `just run` (or `just server` + `just client`), open Map Maker, create/open a draft. (1) Hold middle mouse (or Space + left drag) and pan continuously; scroll the wheel over the map to zoom. Expect smooth camera motion; buttons keep hover highlight and the palette does not flicker. (2) Press V, select a placement, then pan/zoom and press F (Fit): the rotate/duplicate/delete/close toolbar should stay anchored above the selection (it now also follows Fit, which it previously did not). (3) Wheel over the palette still cycles assets. Report any visual difference from before.

# Learn from errors

- Adding one system pushed the `PresentFlow` tuple past Bevy's system-tuple arity limit, giving a misleading `in_set` method-not-found error. Prevention: when a registration tuple is near 20 entries, group related systems in a nested tuple.
- A private marker component in a `pub(in crate::client::flow)` system's query is a `private_interfaces` error at the registration site; match the marker's visibility to the system.
- The per-frame rebuild had been masking a hidden dependency (selection toolbar ↔ camera). Before removing a "rebuild everything" trigger, list which rebuilt nodes read the changed state.

# Deferred

- BRL-cwm9 (todo, related, parent BRL-q5c4): retained updates for discrete value-only Map Maker edits, gated on evidence that one-shot rebuilds cause a problem.

# Playtest feedback (2026-11-13)
- Gameplay playtest accepted for the other plan tickets; the Map Maker was not exercised yet. Ticket stays `doing` awaiting the Map Maker pan/zoom check.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.
