---
id: BRL-0030
title: Separate environment visual catalog from runtime asset preparation
status: done
theme: quality
release:
priority: none
created: 2026-08-28T07:52:25Z
modified: 2026-08-28T11:24:29Z
closed: 2026-08-28T11:24:29Z
revision: 8e396429099260b1
blocks: []
related: []
---

# Description

Refactor `src/client/presentation_3d/environment_assets.rs` so embedded visual/theme catalog definitions and validation are separate from runtime asset handles, imported-scene readiness, material tinting, and scene preparation. Preserve visual output, asset paths, fitting, validation strictness, client-only isolation, and presentation APIs.
