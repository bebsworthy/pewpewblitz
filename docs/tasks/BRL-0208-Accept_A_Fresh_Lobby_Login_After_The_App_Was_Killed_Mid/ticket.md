---
id: BRL-0208
title: Accept a fresh lobby login after the app was killed mid-session
status: todo
theme:
release:
created: 2026-11-14T01:38:35Z
modified: 2026-11-14T01:38:37Z
closed:
status_changed: 2026-11-14T01:38:35Z
history:
- todo@2026-11-14T01:38:35Z
revision: b4e5d673104ff747
blocks: []
related: [BRL-0162]
parent: BRL-0171
---

# Description

Reopening the Android app right after it was force-stopped or killed connects instead of being rejected as AccountInUse until the old session times out.
