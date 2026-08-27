---
id: BRL-0005
title: 'P1: Make ECS app tests fail on unexpected system errors'
status: doing
theme: quality
release:
created: 2026-08-27T18:12:30Z
modified: 2026-08-27T18:48:18Z
closed:
revision: cebb8c1453bfde4d
blocks: []
related: [BRL-0002]
---

# Description

Audit finding TEST-01. Schedule-style Bevy tests can pass after Lightyear or Avian systems fail parameter validation and are skipped. Source: audit/bevy-rust-code-audit-20260827.md.
