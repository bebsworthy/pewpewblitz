---
id: BRL-0208

title: Accept a fresh lobby login after the app was killed mid-session

status: doing
closed:
status_changed: 2026-11-14T04:03:17Z

theme:

release:

created: 2026-11-14T01:38:35Z

blocks: []

related: [BRL-0162, BRL-0212]

parent: BRL-0171

rank: 12517376
---

# Description

Reopening the Android app right after it was force-stopped or killed connects instead of being rejected as AccountInUse until the old session times out.
