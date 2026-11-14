---
id: BRL-fk77

title: Fix float_cmp clippy failure in the balance-lab network build scenario

status: todo
closed:
status_changed: 2026-11-14T10:00:33Z

theme: quality

release:

created: 2026-11-14T10:00:33Z

blocks: []

related: [BRL-qrb8]

parent: BRL-q5c4

rank: 14417920
---

# Description

`cargo clippy -p brawler-client --tests --features network-test,balance-lab -- -D warnings` fails on a pre-existing strict float comparison in `tests/network/builds.rs` (`assert_eq!(server_loadout.fighter_stats.movement_speed, 444.0)`). No canonical `just lint` recipe compiles that combination, so it was never caught; fix the assertion or add the combination to lint.
