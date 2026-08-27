---
id: BRL-0014
title: 'P2: Split combat visual state into focused lifecycles'
status: todo
theme: quality
release:
priority: none
created: 2026-08-27T18:15:30Z
modified: 2026-08-27T20:54:33Z
closed:
revision: d9092ea9a954312c
blocks: []
related: [BRL-0002]
---

# Description

Audit finding PRES-01. update_combat_visual_state owns overhead UI, durable statuses, dash trails, and aim preview in one per-frame system with temporary collections and nested trail lookup. Source: audit/bevy-rust-code-audit-20260827.md.
