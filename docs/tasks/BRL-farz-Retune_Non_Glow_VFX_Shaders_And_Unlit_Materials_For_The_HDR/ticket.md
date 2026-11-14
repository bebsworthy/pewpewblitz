---
id: BRL-farz
was: BRL-0176

title: Retune non-glow VFX shaders and unlit materials for the HDR bloom pipeline

status: done
closed: 2026-11-14T21:10:41Z
status_changed: 2026-11-14T21:10:41Z

theme: core

release:

created: 2026-11-12T22:52:14Z

blocks: []

related: [BRL-1cnh, BRL-3ebg, BRL-m10f, BRL-tpd8]

parent: BRL-q5c4

rank:
---

# Description

Area, status, and trail effects read as intended under HDR tonemapping and never bloom by accident, and inert emissive values are removed.
