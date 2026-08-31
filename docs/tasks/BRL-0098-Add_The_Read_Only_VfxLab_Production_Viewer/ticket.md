---
id: BRL-0098
title: Add the read-only VfxLab production viewer
status: doing
theme:
release:
created: 2026-08-31T08:21:40Z
modified: 2026-08-31T11:44:37Z
closed:
revision: c3efe568f4833465
blocks: [BRL-0099]
related: [BRL-0094, BRL-0096]
---

# Description

Deliver a development-only, read-only VfxLab that renders production VFX recipes and lifecycle scenarios in relative isolation. The lab must reuse the same client catalog, semantic resolver, retained assets, materializers, timelines, degradation rules, determinism policy, cleanup, and evidence schema as gameplay. It provides fast inspection and comparison before any editing workflow, while remaining absent from the headless server and production player flow.
