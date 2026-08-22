import type { WeaponTuning } from "../model/balanceLab";

function variant(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object") return null;
  const first = Object.values(value as Record<string, unknown>)[0];
  return first && typeof first === "object" ? (first as Record<string, unknown>) : null;
}

function firstDamage(recipe: Record<string, unknown>) {
  const bundles = recipe.payload_bundles;
  if (!Array.isArray(bundles)) return null;
  for (const bundle of bundles) {
    if (!bundle || typeof bundle !== "object") continue;
    const effects = (bundle as Record<string, unknown>).effects;
    if (!Array.isArray(effects)) continue;
    for (const effect of effects) {
      const damage = effect && typeof effect === "object" ? variant(effect) : null;
      if (damage && typeof damage.amount === "number") return damage.amount;
    }
  }
  return null;
}

export function WeaponSummary({ weapon }: { weapon: WeaponTuning }) {
  const recipe = weapon.recipe;
  const economy = variant(recipe.economy);
  const delivery = variant(recipe.delivery);
  const cooldown = Number(recipe.fire_cooldown_ticks ?? 0);
  const damage = firstDamage(recipe);
  const capacity = Number(economy?.capacity ?? 0);
  const reloadTicks = Number(economy?.reload_duration_ticks ?? 0);
  const range = Number(delivery?.range ?? delivery?.distance ?? delivery?.reach ?? 0);
  const speed = Number(delivery?.speed ?? 0);
  const shotsPerSecond = cooldown > 0 ? 60 / cooldown : 0;
  return (
    <div className="summary-grid" aria-label={`${weapon.displayName} derived facts`}>
      <span><b>{shotsPerSecond.toFixed(2)}</b> shots/s</span>
      <span><b>{capacity || "—"}</b> capacity</span>
      <span><b>{reloadTicks > 0 ? (reloadTicks / 60).toFixed(2) : "—"}</b> reload s</span>
      <span><b>{damage ?? "—"}</b> base damage</span>
      <span><b>{range || "—"}</b> range</span>
      <span><b>{speed > 0 && range > 0 ? (range / speed).toFixed(2) : "—"}</b> travel s</span>
    </div>
  );
}
