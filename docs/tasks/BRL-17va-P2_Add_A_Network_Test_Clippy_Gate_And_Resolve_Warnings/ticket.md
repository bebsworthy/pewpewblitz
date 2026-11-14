---
id: BRL-17va
was: BRL-0010

title: 'P2: Add a network-test Clippy gate and resolve warnings'

status: done
closed: 2026-08-27T21:50:18Z
status_changed: 2026-08-27T21:50:18Z

theme: quality

release:

created: 2026-08-27T18:15:29Z

blocks: []

related: [BRL-f45c]

parent:

rank:
---

# Description

Audit finding TOOL-01 and backlog item MAINT-NETWORK-TEST-LINT. network-test is checked and tested in CI but omitted from strict Clippy; the audit command found 29 errors. Source: audit/bevy-rust-code-audit-20260827.md.
