---
id: BRL-pcpw
was: BRL-0005

title: 'P1: Make ECS app tests fail on unexpected system errors'

status: done
closed: 2026-08-27T18:54:18Z
status_changed: 2026-08-27T18:54:18Z

theme: quality

release:

created: 2026-08-27T18:12:30Z

blocks: []

related: [BRL-f45c]

parent:

rank:
---

# Description

Audit finding TEST-01. Schedule-style Bevy tests can pass after Lightyear or Avian systems fail parameter validation and are skipped. Source: audit/bevy-rust-code-audit-20260827.md.
