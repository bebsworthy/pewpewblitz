---
id: BRL-086r
was: BRL-0085

title: Decompose Balance Lab authoritative apply transaction

status: done
closed: 2026-08-31T04:51:58Z
status_changed: 2026-08-31T04:51:58Z

theme:

release:

created: 2026-08-31T04:26:00Z

blocks: []

related: [BRL-4ms7]

parent:

rank:
---

# Description

Extract the development-only authoritative Balance Lab apply/restore path into an explicit prepare, persist, commit, restart, and publication transaction while preserving fail-closed rollback, same-tick match reset ordering, schemas, player-visible tuning behavior, and the single fixed-tick Bevy schedule boundary.
