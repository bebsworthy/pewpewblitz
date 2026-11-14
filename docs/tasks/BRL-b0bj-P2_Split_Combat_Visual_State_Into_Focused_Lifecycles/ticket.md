---
id: BRL-b0bj
was: BRL-0014

title: 'P2: Split combat visual state into focused lifecycles'

status: done
closed: 2026-08-27T23:07:28Z
status_changed: 2026-08-27T23:07:28Z

theme: quality

release:

created: 2026-08-27T18:15:30Z

blocks: []

related: [BRL-f45c]

parent:

rank:
---

# Description

Audit finding PRES-01. update_combat_visual_state owns overhead UI, durable statuses, dash trails, and aim preview in one per-frame system with temporary collections and nested trail lookup. Source: audit/bevy-rust-code-audit-20260827.md.
