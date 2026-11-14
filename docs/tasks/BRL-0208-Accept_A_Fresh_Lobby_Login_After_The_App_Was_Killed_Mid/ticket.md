---
id: BRL-0208
title: Accept a fresh lobby login after the app was killed mid-session
status: doing
theme:
release:
created: 2026-11-14T01:38:35Z
modified: 2026-11-14T04:50:54Z
closed:
status_changed: 2026-11-14T04:03:17Z
history:
- todo@2026-11-14T01:38:35Z
- doing@2026-11-14T04:03:17Z
revision: 2e8fb38a0a2580cd
blocks: []
related: [BRL-0162, BRL-0212]
parent: BRL-0171
---

# Description

Reopening the Android app right after it was force-stopped or killed connects instead of being rejected as AccountInUse until the old session times out.
