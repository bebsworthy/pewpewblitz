---
id: BRL-0015

title: 'P2: Consolidate flow scroll and focus geometry helpers'

status: done
closed: 2026-08-27T23:29:19Z
status_changed: 2026-08-27T23:29:19Z

theme: quality

release:

created: 2026-08-27T18:15:31Z

blocks: []

related: [BRL-0002]

parent:

rank:
---

# Description

Audit finding DUP-01. Six scroll systems and six focus-visibility systems repeat wheel normalization, offset clamping, and viewport interval math. Source: audit/bevy-rust-code-audit-20260827.md.
