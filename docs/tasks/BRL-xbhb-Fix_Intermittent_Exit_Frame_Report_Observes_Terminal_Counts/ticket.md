---
id: BRL-xbhb

title: Fix intermittent exit_frame_report_observes_terminal_counts_after_the_shutdown_chain panic under load

status: done
closed: 2026-11-20T00:30:31Z
status_changed: 2026-11-20T00:30:31Z

theme:

release:

created: 2026-11-19T18:19:05Z

blocks: []

related: [BRL-stws]

parent:

rank:
---

# Description

The server diagnostics test sometimes panics inside a FixedMain system during a loaded full just test run; find the panic and make the test deterministic.
