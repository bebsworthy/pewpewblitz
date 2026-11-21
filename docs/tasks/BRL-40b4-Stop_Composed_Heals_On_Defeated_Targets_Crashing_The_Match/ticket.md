---
id: BRL-40b4

title: Stop composed heals on defeated targets crashing the match worker

status: done
closed: 2026-11-21T15:41:53Z
status_changed: 2026-11-21T15:41:53Z

theme: core

release:

created: 2026-11-21T15:06:18Z

blocks: []

related: [BRL-35ns, BRL-wrer]

parent:

rank:
---

# Description

A composed heal that lands on a target already defeated earlier in the same combat batch crashes the authoritative match worker; it should be skipped without consuming a combat event.
