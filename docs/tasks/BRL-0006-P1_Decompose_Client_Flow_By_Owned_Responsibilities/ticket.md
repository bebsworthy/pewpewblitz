---
id: BRL-0006
title: 'P1: Decompose client flow by owned responsibilities'
status: doing
theme: quality
release:
priority: none
created: 2026-08-27T18:15:28Z
modified: 2026-08-27T20:28:55Z
closed:
revision: fe7b9b3cb707422f
blocks: []
related: [BRL-0002]
---

# Description

Audit finding ARCH-01. client/flow.rs combines connection orchestration, persistence, state reduction, multiple screens, navigation, and shared UI mechanics in one subsystem. Source: audit/bevy-rust-code-audit-20260827.md.
