---
id: BRL-r8ev

title: Apply the cheaper dev debuginfo profile and a just sweep recipe

status: todo
closed:
status_changed: 2026-11-14T10:00:33Z

theme: quality

release:

created: 2026-11-14T10:00:33Z

blocks: []

related: [BRL-0170]

parent: BRL-0171

rank: 14352384
---

# Description

Land the cheaper dev profile that BRL-0170's spec assumed was already in place (`debug = "line-tables-only"` for workspace crates, `debug = false` for dependencies) and a `just sweep` stale-artifact recipe, then re-measure incremental rebuild time and `target/` size against the BRL-0170 tables.
