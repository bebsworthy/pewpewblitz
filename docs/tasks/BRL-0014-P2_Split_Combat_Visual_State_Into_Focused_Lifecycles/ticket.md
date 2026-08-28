---
id: BRL-0014
title: 'P2: Split combat visual state into focused lifecycles'
status: done
theme: quality
release:
priority: none
created: 2026-08-27T18:15:30Z
modified: 2026-08-27T23:07:28Z
closed: 2026-08-27T23:07:28Z
revision: 06aead9c2f500b9d
blocks: []
related: [BRL-0002]
---

# Description

Audit finding PRES-01. update_combat_visual_state owns overhead UI, durable statuses, dash trails, and aim preview in one per-frame system with temporary collections and nested trail lookup. Source: audit/bevy-rust-code-audit-20260827.md.
