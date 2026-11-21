---
id: BRL-c96b

title: Author Last Stand boundary and boost rules per game type

status: backlog
closed:
status_changed: 2026-11-21T16:36:46Z

theme: core

release:

created: 2026-11-21T16:36:46Z

blocks: []

related: [BRL-5ss9]

parent:

rank: 12189696
---

# Description

Move Last Stand's code-owned rules (`LastStandRules::default()` in `src/matchplay/last_stand.rs`: the four boundary phases, pulse interval and damage, boost stack limit 6, pickup radius) into validated operator config per game type, as the BRL-5ss9 owner decision asked ("stack value, cap, chest count, and chest health are authored per game type"). BRL-5ss9 shipped them in code like Zombie's rules, so no control IPC or manifest bump was needed; this ticket adds the `mode_policies.last_stand` policy, the IPC/manifest carriage, and Balance Lab tuning once playtests show which values need tuning.
