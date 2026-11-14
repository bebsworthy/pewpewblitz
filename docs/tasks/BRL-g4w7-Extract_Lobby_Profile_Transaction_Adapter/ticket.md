---
id: BRL-g4w7
was: BRL-0087

title: Extract lobby profile transaction adapter

status: done
closed: 2026-08-31T05:28:20Z
status_changed: 2026-08-31T05:28:20Z

theme:

release:

created: 2026-08-31T05:12:32Z

blocks: []

related: [BRL-4ms7]

parent:

rank:
---

# Description

Extract the server lobby's profile-backed admission and authenticated profile-command bridge from the lobby composition root into one private transaction adapter while preserving exact validation/rejection precedence, load-before-command processing, queue-lock policy, deferred-command visibility, protocol shapes, and server-authoritative behavior.
