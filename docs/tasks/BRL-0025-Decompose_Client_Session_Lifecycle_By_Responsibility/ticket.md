---
id: BRL-0025

title: Decompose client session lifecycle by responsibility

status: done
closed: 2026-08-28T08:58:03Z
status_changed: 2026-08-28T08:58:03Z

theme: quality

release:

created: 2026-08-28T07:52:25Z

blocks: []

related: [BRL-0022, BRL-0031]

parent:

rank:
---

# Description

Refactor `src/client/session.rs` into focused client-session owners for connection materialization, compatibility and identity handshake, routed transitions, match commands/loading, automation, and terminal shutdown. Preserve the `ClientNetworkPlugin` API, explicit schedule ordering, player-visible behavior, protocol traffic, recovery semantics, and role-feature isolation.
