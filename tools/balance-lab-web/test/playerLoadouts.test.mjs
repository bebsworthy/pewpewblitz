import assert from "node:assert/strict";
import test from "node:test";
import { weaponModifierLabels } from "../src/pages/balance-lab/lib/playerLoadouts.ts";

test("effective weapon modifiers omit zero values and preserve gameplay units", () => {
  const labels = weaponModifierLabels({
    capacity: { flat: 2, percentBasisPoints: 0 },
    damage: { flat: 0, percentBasisPoints: 1_500 },
    fireInterval: { flat: 0, percentBasisPoints: 0 },
    refillInterval: { flat: -3, percentBasisPoints: -2_000 },
    reachMilliunits: { flat: 32_000, percentBasisPoints: 0 },
    slow: { penaltyBasisPoints: 1_500, durationTicks: 36 },
  });

  assert.deepEqual(labels, [
    "Capacity +2 ammo",
    "Damage +15%",
    "Refill interval -3 ticks · -20%",
    "Reach +32 units",
    "Slow 15% · 36 ticks",
  ]);
});

test("an unmodified weapon has no modifier labels", () => {
  const zero = { flat: 0, percentBasisPoints: 0 };
  assert.deepEqual(
    weaponModifierLabels({
      capacity: zero,
      damage: zero,
      fireInterval: zero,
      refillInterval: zero,
      reachMilliunits: zero,
      slow: null,
    }),
    [],
  );
});
