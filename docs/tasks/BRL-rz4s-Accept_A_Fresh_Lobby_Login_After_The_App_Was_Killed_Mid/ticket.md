---
id: BRL-rz4s
was: BRL-0208

title: Accept a fresh lobby login after the app was killed mid-session

status: done
closed: 2026-11-14T21:10:41Z
status_changed: 2026-11-14T21:10:41Z

theme:

release:

created: 2026-11-14T01:38:35Z

blocks: []

related: [BRL-g57a, BRL-thwh]

parent: BRL-q5c4

rank:
---

# Description

Reopening the Android app right after it was force-stopped or killed connects instead of being rejected as AccountInUse until the old session times out.
