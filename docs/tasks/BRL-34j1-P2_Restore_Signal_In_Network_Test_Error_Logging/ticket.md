---
id: BRL-34j1
was: BRL-0011

title: 'P2: Restore signal in network-test error logging'

status: done
closed: 2026-08-27T22:01:49Z
status_changed: 2026-08-27T22:01:49Z

theme: quality

release:

created: 2026-08-27T18:15:29Z

blocks: []

related: [BRL-f45c]

parent:

rank:
---

# Description

Audit finding OBS-01. Successful impairment and soak tests emit thousands of expected Lightyear ERROR lines plus repeated logger-installation errors, obscuring unexpected ECS failures. Source: audit/bevy-rust-code-audit-20260827.md.
