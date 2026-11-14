---
id: BRL-0176

title: Retune non-glow VFX shaders and unlit materials for the HDR bloom pipeline

status: todo
closed:
status_changed: 2026-11-12T22:52:14Z

theme: core

release:

created: 2026-11-12T22:52:14Z

blocks: []

related: [BRL-0146]

parent: BRL-0171

rank: 917504
---

# Description

Area, status, and trail effects read as intended under HDR tonemapping and never bloom by accident, and inert emissive values are removed.
