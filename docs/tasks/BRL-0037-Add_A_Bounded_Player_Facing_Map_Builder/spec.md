# Outcome

A player can author a bounded map recipe from the existing catalog, receive authoritative validation, save and reopen it, and launch a server-authoritative Practice playtest from the ordinary product flow.

# First slice

- Add a Dashboard map-builder flow with supported mode and theme selection.
- Support bounded grid placement, deletion, quarter-turn rotation, surface/feature/decoration/marker slots, team spawns, and the selected mode's typed anchors.
- Show dimensions, placement limits, validation errors, required anchors, spawn capacity, and playable preview using current catalog facts.
- Save recipes with stable local identity and schema, reopen them, duplicate them, and delete them through explicit user actions.
- Submit the recipe through an explicit bounded server validation/admission boundary before Practice allocation; the client never decides legality.
- Launch the accepted recipe through the existing routed authoritative match worker and ordinary results/return flow.
- Built-in and authored recipes use the same resolver and runtime installation contracts.

# Boundaries

- No asset upload, executable mode rules, procedural generation, publishing, discovery, moderation, collaboration, browser editor, or map-bundle provisioning.
- The editor does not infer gameplay from visuals or bypass server-known catalogs.

# Acceptance criteria

- Create/edit/validate/save/reopen/delete/playtest flows work with keyboard, mouse, and controller where the Dashboard supports them.
- Invalid dimensions, slots, anchors, spawns, bounds, capacities, identities, and unsupported content fail closed with actionable messages.
- Persistence and routed handoff recover safely from stale catalogs, schema mismatch, cancellation, disconnect, and restart.
- Representative Wipeout, Hot Zone, and Heist recipes pass automated and native authoring/playtest evidence.
