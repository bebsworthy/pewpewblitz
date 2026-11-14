---
id: BRL-c2e0
was: BRL-0006

title: 'P1: Decompose client flow by owned responsibilities'

status: done
closed: 2026-08-27T21:24:32Z
status_changed: 2026-08-27T21:24:32Z

theme: quality

release:

created: 2026-08-27T18:15:28Z

blocks: []

related: [BRL-f45c, BRL-v6yb]

parent:

rank:
---

# Description

Audit finding ARCH-01. client/flow.rs combines connection orchestration, persistence, state reduction, multiple screens, navigation, and shared UI mechanics in one subsystem. Source: audit/bevy-rust-code-audit-20260827.md.
