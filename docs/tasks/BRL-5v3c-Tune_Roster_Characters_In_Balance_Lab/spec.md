# Outcome

Balance Lab gains a Characters category over the character catalog from BRL-egn6. It edits per-character stats, the signature weapon base and authored parts, and the 2 ultimate options and 2 passive options. It reuses the existing schema-driven validation and applied/default state, and the primitives (weapons, ultimates, passives, parts) stay editable as they are today.

# Notes

- Blocked by BRL-egn6.
- Characters are the natural columns for BRL-ytx3's comparison matrices. Coordinate with that ticket rather than building a second layout.
- Verify with `just lint`, `just test` and the Balance Lab web build. The owner then tunes the roster on a native client.
