export function fieldLabel(value: string) {
  return value
    .replace(/_/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/^./, (letter) => letter.toUpperCase());
}

type NumericPath = (string | number)[];

function isTerrainBrushRadius(path: NumericPath) {
  return String(path.at(-1)) === "radius" && path.some((part) => part === "world_effects");
}

function isBarrelField(path: NumericPath, field: string) {
  return path.includes("barrel") && String(path.at(-1)) === field;
}

function isHeistField(path: NumericPath, field: string) {
  return path.includes("heist") && String(path.at(-1)) === field;
}

function isChestField(path: NumericPath, field: string) {
  return path.includes("chest") && String(path.at(-1)) === field;
}

export function numberSpec(key: string, value: number, path: NumericPath) {
  const lower = key.toLowerCase();
  const integer = Number.isInteger(value);
  if (isTerrainBrushRadius(path)) return { min: 8, max: 128, step: 4 };
  if (isBarrelField(path, "maximum_health")) return { min: 1, max: 1000, step: 1 };
  if (isHeistField(path, "safeMaximumHealth")) return { min: 100, max: 20000, step: 100 };
  if (isChestField(path, "maximum_health")) return { min: 1, max: 1000, step: 1 };
  if (isChestField(path, "restoration")) return { min: 1, max: 1000, step: 1 };
  if (isChestField(path, "collection_radius_world_units")) return { min: 8, max: 64, step: 1 };
  if (isChestField(path, "lifetime_ticks")) return { min: 60, max: 3600, step: 60 };
  if (isBarrelField(path, "radius_world_units")) return { min: 1, max: 512, step: 1 };
  if (isBarrelField(path, "maximum_targets") || isBarrelField(path, "maximum_chain_reactions")) {
    return { min: 1, max: 16, step: 1 };
  }
  if (lower.includes("multiplier") || lower.includes("scale")) {
    return { min: 0, max: 2, step: 0.01 };
  }
  if (lower.includes("angle")) return { min: 1, max: 180, step: 0.5 };
  if (lower.includes("health") || lower.includes("damage")) {
    return { min: 1, max: 1000, step: 1 };
  }
  if (lower === "movement_speed") {
    return { min: 80, max: 1200, step: integer ? 1 : 0.1 };
  }
  if (lower === "reveal_proximity_radius") {
    return { min: 32, max: 1024, step: integer ? 1 : 0.1 };
  }
  if (lower.includes("capacity")) return { min: 1, max: 32, step: 1 };
  if (lower.includes("target") || lower.includes("count")) {
    return { min: 1, max: 16, step: 1 };
  }
  if (lower.includes("tick")) return { min: 1, max: 3600, step: 1 };
  if (lower.includes("speed") || lower.includes("range") || lower.includes("distance")) {
    return { min: 1, max: 4096, step: integer ? 1 : 0.1 };
  }
  if (lower.includes("radius")) return { min: 1, max: 512, step: integer ? 1 : 0.1 };
  return { min: 0, max: Math.max(100, Math.ceil(value * 2)), step: integer ? 1 : 0.1 };
}

export function constraintHint(path: NumericPath) {
  if (isTerrainBrushRadius(path)) return "Engine safety: 8–128 world units, aligned to the 4-unit terrain grid.";
  if (path.includes("chest")) return "Chest bounds: health/restoration 1–1000, radius 8–64, lifetime 60–3600 ticks.";
  return null;
}

export function secondsFor(key: string, value: number) {
  return key.toLowerCase().includes("tick") ? `${(value / 60).toFixed(2)} s` : null;
}
