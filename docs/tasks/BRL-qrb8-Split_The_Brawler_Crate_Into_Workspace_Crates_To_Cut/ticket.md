---
id: BRL-qrb8
was: BRL-0170

title: Split the brawler crate into workspace crates to cut rebuild cost

status: doing
closed:
status_changed: 2026-11-14T08:40:42Z

theme: quality

release:

created: 2026-10-23T17:18:22Z

blocks: []

related: [BRL-fk77, BRL-nrab, BRL-r8ev]

parent: BRL-q5c4

rank: 12648448
---

# Description

Measure, then split the single brawler package into a small number of workspace crates along its existing execution-role boundaries. An edit to one side (for example client presentation) should no longer recompile the gameplay core under every feature set, and both incremental edit-to-test time and target/ disk use should drop measurably.
