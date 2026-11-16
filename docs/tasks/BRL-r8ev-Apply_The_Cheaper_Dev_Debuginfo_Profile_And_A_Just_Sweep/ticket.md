---
id: BRL-r8ev

title: Apply the cheaper dev debuginfo profile and a just sweep recipe

status: doing
closed:
status_changed: 2026-11-16T23:32:46Z

theme: quality

release:

created: 2026-11-14T10:00:33Z

blocks: []

related: [BRL-qrb8]

parent: BRL-q5c4

rank: 11337728
---

# Description

Land the cheaper dev profile that BRL-qrb8's spec assumed was already in place (`debug = "line-tables-only"` for workspace crates, `debug = false` for dependencies) and a `just sweep` stale-artifact recipe, then re-measure incremental rebuild time and `target/` size against the BRL-qrb8 tables.
