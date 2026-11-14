---
id: BRL-ambe
was: BRL-0029

title: 'Separate diagnostics sampling, measurement windows, and closeout'

status: done
closed: 2026-08-28T11:02:23Z
status_changed: 2026-08-28T11:02:23Z

theme: quality

release:

created: 2026-08-28T07:52:25Z

blocks: []

related: [BRL-xtbw]

parent:

rank:
---

# Description

Refactor `src/diagnostics/process.rs` into focused owners for process sampling, authoritative common-window evidence, terminal closeout assembly/writing, and environment-derived run identity. Preserve observational-only behavior, schemas, report ordering, feature gates, terminal schedule edges, and evidence semantics.
