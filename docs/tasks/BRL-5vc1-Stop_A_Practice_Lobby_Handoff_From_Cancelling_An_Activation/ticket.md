---
id: BRL-5vc1

title: Stop a Practice lobby handoff from cancelling an activation whose match runs on

status: backlog
closed:
status_changed: 2026-11-15T00:41:32Z

theme:

release:

created: 2026-11-15T00:41:32Z

blocks: []

related: [BRL-k0tr, BRL-yyeg]

parent: BRL-q5c4

rank: 14614528
---

# Description

Found during BRL-yyeg: a Practice reservation's lobby session closes at its own match handoff, and `cleanup_disconnected_sessions` then calls `cancel_product_allocation` and pushes `CancelActivation`, so the supervisor sends `ActivationDissolved` to the lobby and the match worker while the match runs on. It is harmless today (the worker ignores it and BRL-yyeg seats are released only on `MatchEnded`), but the fact is wrong. Clear the Practice formation at grant delivery or ignore the loss once the grant was delivered.
