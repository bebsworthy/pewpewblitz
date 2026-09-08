## Scope
Audit existing player screen contracts and source; prepare a reusable handoff under docs/design/player-shell/. Cover launch/recovery, Dashboard, brawler list/details/create/abilities/weapon equipment, game selection, queue/loading/results, settings, credits and contextual overlays. Exclude gameplay HUD and developer tools. Propose a colorful animated coherent direction without changing navigation or gameplay authority.

## Acceptance
- Screen map, purpose, required content/actions, important states and source references.
- Shared art direction and individually usable screen prompts; staged generation and cross-screen reference workflow.
- Motion/interaction and responsive guidance, with current features distinguished from proposed presentation.
- Deliverable format contract explaining PNG, SVG, editable layouts, font/icon sources, optional Blender/GLB and animation references; no assumption imaging models can produce valid editable assets.
- Review pack for coverage, consistency and implementation feasibility; record evidence and sync.

## Verification
Documentation/source audit and internal link/coverage checks. No runtime code changed; no build or playtest required. Style approval and actual generation/implementation are subsequent work.


## Delivery and verification — 2026-09-07
Delivered docs/design/player-shell/{README,screen-map,generation-brief,motion,delivery-contract}.md: 18 screen/overlay families with matching prompts, canonical navigation, three Dashboard exploration directions, anchor-screen generation workflow, proposed shared style rules, responsive studies, motion/reduced-motion behavior and implementation asset contract. A standalone text export containing all five documents was produced in the task's visualization output directory for upload to another model.

Source audit: src/client/flow/screens, src/client/shell.rs, src/client/settings/ui.rs, src/client/assets.rs and durable product/UX/art documents. Repowise returned weak/stale retrieval; direct source was used. No fresh native visual inspection or images generated. Scope is design handoff only; style selection and implementation remain subsequent work.

Decisions: use approved PNGs plus written specs as minimum; editable SVG only for real geometric assets, existing fighter models for live previews, optional Blender/GLB for justified new stage props. Full-screen grouped settings is explicitly proposed rather than claimed current behavior. Existing quiet-shell direction is identified as the baseline being evolved, not silently overwritten. Settings controls reflect implementation rather than aspirational docs; generic weapon slots remain generic; cooperative results are conditional on advertised content.

Verification: Python check passed every local Markdown link and all 18 screen-to-prompt pairs. git diff --check for the new documentation reported no errors. Manual consistency review checked no Title layer, draft/commit semantics, honest queue/loading status, generic equipment slots, actual settings controls, missing/pending/error variants and no invented progression. Runtime code untouched; builds/native playtests are not applicable to this handoff. Unrelated working-tree changes preserved.

Learning: broad documentation reads were too noisy; focused source slices exposed the important differences between aspirational settings and current controls. Future visual briefs should audit concrete screen actions before writing prompts and separate visual exploration outputs from production assets. No new skill is warranted for this one-off design brief.
