---
id: BRL-sbep

title: Shut the server down gracefully on SIGTERM without orphaning workers

status: backlog
closed:
status_changed: 2026-11-21T06:28:00Z

theme:

release:

created: 2026-11-20T19:50:12Z

blocks: [BRL-n7c8]

related: [BRL-jjwa, BRL-pweq]

parent: BRL-9wp9

rank: 11010048
---

# Description

Stopping the server the way a service manager does drains matches and leaves no worker processes behind.
