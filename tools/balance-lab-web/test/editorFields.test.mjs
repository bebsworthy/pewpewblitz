import assert from "node:assert/strict";
import test from "node:test";
import {
  changedFields,
  displayNumber,
  fieldFromServerError,
  formatFieldNumber,
  pathKey,
  readAtPath,
  replaceAtPath,
  toStoredNumber,
  validateDisplayNumber,
} from "../src/pages/balance-lab/lib/editorFields.ts";

const snapshot = {
  schemaVersion: 3,
  fighterProfiles: {
    default: { maximum_health: 100 },
  },
  ultimates: [{ parameters: { RevealScan: { maximum_range_milliunits: 384_000 } } }],
};

const health = {
  path: ["fighterProfiles", "default", "maximum_health"],
  storageKind: "integer",
  storageScale: 1,
  minimum: 1,
  maximum: 65_535,
  minimumExclusive: false,
  step: 1,
  unit: "health",
};

const range = {
  path: ["ultimates", 0, "parameters", "RevealScan", "maximum_range_milliunits"],
  storageKind: "integer",
  storageScale: 1_000,
  minimum: 0.001,
  maximum: 4_096,
  minimumExclusive: false,
  step: 0.001,
  unit: "world units",
};

const weaponFlight = {
  ...health,
  path: ["weapons", 2, "recipe", "delivery", "Lobbed", "max_flight_ticks"],
  storageScale: 60,
  minimum: 0.1,
  maximum: 10,
  step: 1 / 60,
  unit: "s",
};

const weaponHeal = {
  ...health,
  path: ["weapons", 6, "recipe", "payload_bundles", 0, "effects", 1, "Heal", "amount"],
  maximum: 1_000,
};

test("paths traverse arrays and objects without mutating the snapshot", () => {
  assert.equal(readAtPath(snapshot, range.path), 384_000);
  const changed = replaceAtPath(snapshot, range.path, 512_000);
  assert.equal(readAtPath(changed, range.path), 512_000);
  assert.equal(readAtPath(snapshot, range.path), 384_000);
  assert.equal(pathKey(range.path), JSON.stringify(range.path));
});

test("editor scales authoritative milliunits for display and storage", () => {
  assert.equal(displayNumber(snapshot, range), 384);
  assert.equal(toStoredNumber(384.125, range), 384_125);
});

test("ordinary decimal seconds snap to an authoritative tick", () => {
  const ticks = {
    ...health,
    storageScale: 60,
    minimum: 1 / 60,
    step: 1 / 60,
    unit: "s",
  };
  assert.equal(validateDisplayNumber(0.333333, ticks), null);
  assert.equal(toStoredNumber(0.333333, ticks), 20);
  assert.equal(validateDisplayNumber(0.17, ticks), null);
  assert.equal(toStoredNumber(0.17, ticks), 10);
  assert.equal(formatFieldNumber(10 / 60, ticks), "0.17");
});

test("authoritative bounds drive inline validation", () => {
  assert.equal(validateDisplayNumber(100, health), null);
  assert.match(validateDisplayNumber(100.5, health), /increments of/);
  assert.match(validateDisplayNumber(0, health), /at least 1 health/);
  assert.equal(validateDisplayNumber(65_535, health), null);
  assert.match(validateDisplayNumber(65_536, health), /at most 65535 health/);
});

test("corrected weapon policy bounds drive inline validation", () => {
  assert.match(validateDisplayNumber(0.09, weaponFlight), /at least 0.1 s/);
  assert.equal(validateDisplayNumber(0.1, weaponFlight), null);
  assert.equal(toStoredNumber(0.1, weaponFlight), 6);

  assert.equal(validateDisplayNumber(1_000, weaponHeal), null);
  assert.match(validateDisplayNumber(1_001, weaponHeal), /at most 1000 health/);
});

test("changed field count ignores non-editable metadata", () => {
  const metadataOnly = structuredClone(snapshot);
  metadataOnly.schemaVersion = 99;
  assert.equal(changedFields([health, range], metadataOnly, snapshot).length, 0);

  const edited = replaceAtPath(snapshot, health.path, 125);
  assert.deepEqual(changedFields([health, range], edited, snapshot), [health]);
});

test("a field-specific server rejection maps back to its descriptor", () => {
  const result = fieldFromServerError(
    "field /fighterProfiles/default/maximum_health: rejected by server",
    [health, range],
  );
  assert.equal(result?.field, health);
  assert.equal(result?.message, "rejected by server");
  assert.equal(fieldFromServerError("stale applied revision", [health]), null);
});
