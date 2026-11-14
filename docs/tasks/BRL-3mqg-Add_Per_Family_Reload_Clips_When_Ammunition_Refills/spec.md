# Context
BRL-q29d scoped reload clips as optional and did not add them. First confirm whether the replicated weapon economy exposes a refill/reload state the client can read without a protocol change; if not, decide whether a reload read is worth one.

# Scope
- reload_<family> clips in build_riff.py using the BRL-q29d weapon placement + IK bake.
- A select_motion priority slot below attack and above run.

# Acceptance
- Reload reads at gameplay scale for each family; no gameplay or protocol change unless approved.
