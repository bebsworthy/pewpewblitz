import type { ScalarModifier, WeaponModifiers } from "../model/balanceLab";

function number(value: number) {
  return Number.isInteger(value) ? String(value) : String(Number(value.toFixed(3)));
}

function signed(value: number) {
  return `${value > 0 ? "+" : ""}${number(value)}`;
}

function scalarParts(modifier: ScalarModifier, flatScale = 1, flatUnit = "") {
  const parts: string[] = [];
  if (modifier.flat !== 0) {
    parts.push(`${signed(modifier.flat / flatScale)}${flatUnit}`);
  }
  if (modifier.percentBasisPoints !== 0) {
    parts.push(`${signed(modifier.percentBasisPoints / 100)}%`);
  }
  return parts;
}

export function weaponModifierLabels(modifiers: WeaponModifiers) {
  const labels: string[] = [];
  const add = (label: string, values: string[]) => {
    if (values.length > 0) labels.push(`${label} ${values.join(" · ")}`);
  };
  add("Capacity", scalarParts(modifiers.capacity, 1, " ammo"));
  add("Damage", scalarParts(modifiers.damage, 1, " damage"));
  add("Fire interval", scalarParts(modifiers.fireInterval, 1, " ticks"));
  add("Refill interval", scalarParts(modifiers.refillInterval, 1, " ticks"));
  add("Reach", scalarParts(modifiers.reachMilliunits, 1_000, " units"));
  if (modifiers.slow) {
    labels.push(
      `Slow ${number(modifiers.slow.penaltyBasisPoints / 100)}% · ${modifiers.slow.durationTicks} ticks`,
    );
  }
  if (modifiers.cold != null) labels.push(`Cold +${modifiers.cold}`);
  if (modifiers.poison) labels.push(`Poison ${modifiers.poison.damagePerTick}/${modifiers.poison.tickInterval}t · ${modifiers.poison.durationTicks} ticks`);
  if (modifiers.fire) labels.push(`Fire ${modifiers.fire.damagePerTick}/${modifiers.fire.tickInterval}t · ${modifiers.fire.durationTicks} ticks`);
  if (modifiers.heal != null) labels.push(`Healing ${modifiers.heal}`);
  return labels;
}
