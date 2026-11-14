---
id: BRL-0031

title: Restore focused routed smoke terminal result acceptance

status: done
closed: 2026-09-09T09:49:22Z
status_changed: 2026-09-09T09:49:22Z

theme: quality

release:

created: 2026-08-28T08:55:40Z

blocks: []

related: [BRL-0025, BRL-0026, BRL-0029]

parent:

rank:
---

# Description

Restore the canonical headless `scripts/network-routed.sh` lobby-to-match-to-fresh-lobby smoke so a verification-rules match worker's successful terminal result is accepted by the supervisor, routes are revoked normally, and both clients return to an authenticated fresh lobby instead of timing out with `WorkerExitMismatch`.
