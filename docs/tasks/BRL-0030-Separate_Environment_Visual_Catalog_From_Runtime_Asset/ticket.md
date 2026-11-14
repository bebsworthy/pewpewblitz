---
id: BRL-0030

title: Separate environment visual catalog from runtime asset preparation

status: done
closed: 2026-08-28T11:24:29Z
status_changed: 2026-08-28T11:24:29Z

theme: quality

release:

created: 2026-08-28T07:52:25Z

blocks: []

related: []

parent:

rank:
---

# Description

Refactor `src/client/presentation_3d/environment_assets.rs` so embedded visual/theme catalog definitions and validation are separate from runtime asset handles, imported-scene readiness, material tinting, and scene preparation. Preserve visual output, asset paths, fitting, validation strictness, client-only isolation, and presentation APIs.
