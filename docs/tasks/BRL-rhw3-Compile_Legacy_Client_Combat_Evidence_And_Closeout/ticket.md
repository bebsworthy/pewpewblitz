---
id: BRL-rhw3

title: 'Compile legacy client combat evidence and closeout projection out of the shared crate''s client role'

status: done
closed: 2026-11-19T14:23:44Z
status_changed: 2026-11-19T14:23:44Z

theme: quality

release:

created: 2026-11-14T21:47:00Z

blocks: [BRL-5fk5]

related: [BRL-3p70, BRL-stws]

parent: BRL-q5c4

rank:
---

# Description

The shared `brawler` crate's `client` role no longer compiles the legacy direct-UDP client combat evidence (`ClientCombatEvidencePlugin`, `ClientCombatObservation`, the ready-file report) or the client closeout projection, rather than relying on the player binary's linker dead-stripping.
