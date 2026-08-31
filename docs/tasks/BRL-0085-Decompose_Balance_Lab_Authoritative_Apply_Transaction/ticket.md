---
id: BRL-0085
title: Decompose Balance Lab authoritative apply transaction
status: done
theme:
release:
created: 2026-08-31T04:26:00Z
modified: 2026-08-31T04:51:58Z
closed: 2026-08-31T04:51:58Z
revision: b595b5a36b08dc94
blocks: []
related: [BRL-0070]
---

# Description

Extract the development-only authoritative Balance Lab apply/restore path into an explicit prepare, persist, commit, restart, and publication transaction while preserving fail-closed rollback, same-tick match reset ordering, schemas, player-visible tuning behavior, and the single fixed-tick Bevy schedule boundary.
