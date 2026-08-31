---
id: BRL-0087
title: Extract lobby profile transaction adapter
status: done
theme:
release:
created: 2026-08-31T05:12:32Z
modified: 2026-08-31T05:28:20Z
closed: 2026-08-31T05:28:20Z
revision: a0c67f57dcb6463f
blocks: []
related: [BRL-0070]
---

# Description

Extract the server lobby's profile-backed admission and authenticated profile-command bridge from the lobby composition root into one private transaction adapter while preserving exact validation/rejection precedence, load-before-command processing, queue-lock policy, deferred-command visibility, protocol shapes, and server-authoritative behavior.
