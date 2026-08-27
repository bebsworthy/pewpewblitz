---
id: BRL-0015
title: 'P2: Consolidate flow scroll and focus geometry helpers'
status: backlog
theme: quality
release:
created: 2026-08-27T18:15:31Z
modified: 2026-08-27T18:15:33Z
closed:
revision: 3b02a9041da07ca1
blocks: []
related: [BRL-0002]
---

# Description

Audit finding DUP-01. Six scroll systems and six focus-visibility systems repeat wheel normalization, offset clamping, and viewport interval math. Source: audit/bevy-rust-code-audit-20260827.md.
