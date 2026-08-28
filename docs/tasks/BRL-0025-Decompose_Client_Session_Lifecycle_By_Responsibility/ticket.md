---
id: BRL-0025
title: Decompose client session lifecycle by responsibility
status: done
theme: quality
release:
priority: none
created: 2026-08-28T07:52:25Z
modified: 2026-08-28T08:58:03Z
closed: 2026-08-28T08:58:03Z
revision: 218bdd1249b6b899
blocks: []
related: [BRL-0022, BRL-0031]
---

# Description

Refactor `src/client/session.rs` into focused client-session owners for connection materialization, compatibility and identity handshake, routed transitions, match commands/loading, automation, and terminal shutdown. Preserve the `ClientNetworkPlugin` API, explicit schedule ordering, player-visible behavior, protocol traffic, recovery semantics, and role-feature isolation.
