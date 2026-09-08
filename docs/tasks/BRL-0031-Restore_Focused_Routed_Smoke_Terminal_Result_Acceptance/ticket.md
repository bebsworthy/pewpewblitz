---
id: BRL-0031
title: Restore focused routed smoke terminal result acceptance
status: backlog
theme: quality
release:
created: 2026-08-28T08:55:40Z
modified: 2026-09-08T18:30:19Z
closed:
revision: 939d6a7ea51586c8
blocks: []
related: [BRL-0025, BRL-0026, BRL-0029]
---

# Description

Restore the canonical headless `scripts/network-routed.sh` lobby-to-match-to-fresh-lobby smoke so a verification-rules match worker's successful terminal result is accepted by the supervisor, routes are revoked normally, and both clients return to an authenticated fresh lobby instead of timing out with `WorkerExitMismatch`.
