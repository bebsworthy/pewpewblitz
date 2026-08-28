---
id: BRL-0029
title: 'Separate diagnostics sampling, measurement windows, and closeout'
status: done
theme: quality
release:
priority: none
created: 2026-08-28T07:52:25Z
modified: 2026-08-28T11:02:23Z
closed: 2026-08-28T11:02:23Z
revision: 00a5c9155af25dca
blocks: []
related: [BRL-0031]
---

# Description

Refactor `src/diagnostics/process.rs` into focused owners for process sampling, authoritative common-window evidence, terminal closeout assembly/writing, and environment-derived run identity. Preserve observational-only behavior, schemas, report ordering, feature gates, terminal schedule edges, and evidence semantics.
