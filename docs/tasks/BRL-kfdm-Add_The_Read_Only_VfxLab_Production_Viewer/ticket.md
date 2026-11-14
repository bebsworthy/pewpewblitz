---
id: BRL-kfdm
was: BRL-0098

title: Add the read-only VfxLab production viewer

status: done
closed: 2026-08-31T12:35:12Z
status_changed: 2026-08-31T12:35:12Z

theme:

release:

created: 2026-08-31T08:21:40Z

blocks: [BRL-6akb]

related: [BRL-08sz, BRL-hheg, BRL-pzkk]

parent:

rank:
---

# Description

Deliver a development-only, read-only VfxLab that renders production VFX recipes and lifecycle scenarios in relative isolation. The lab must reuse the same client catalog, semantic resolver, retained assets, materializers, timelines, degradation rules, determinism policy, cleanup, and evidence schema as gameplay. It provides fast inspection and comparison before any editing workflow, while remaining absent from the headless server and production player flow.
