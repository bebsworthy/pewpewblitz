---
id: BRL-7k07

title: Report match telemetry for more than two teams

status: backlog
closed:
status_changed: 2026-11-21T16:36:46Z

theme: core

release:

created: 2026-11-21T16:36:46Z

blocks: []

related: [BRL-5ss9]

parent:

rank: 12386304
---

# Description

Common match telemetry still keeps two-team arrays (`credited_defeats_by_team: [u16; 2]`, team A/B process samples), so with Last Stand's six solo teams (BRL-5ss9) defeats by teams 2–5 are not counted per team (the gates skip `team.0 > 1`). Generalise the per-team telemetry to `MAX_MATCH_TEAMS` where the process report and Balance Lab samples need it.
