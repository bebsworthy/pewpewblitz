import assert from "node:assert/strict";
import test from "node:test";
import {
  canonicalDifferences,
  formatCanonicalDifferences,
} from "../src/pages/balance-lab/lib/canonicalDiff.ts";

const field = {
  path: ["fighterProfiles", "default", "maximum_health"],
  section: "fighters",
  subjectKey: "default",
  subjectLabel: "Default",
  group: "Core stats",
  label: "Maximum health",
  storageKind: "integer",
  storageScale: 1,
  minimum: 1,
  maximum: 65_535,
  minimumExclusive: false,
  step: 1,
  control: "number",
  unit: "health",
};

const baseline = {
  fighterProfiles: { default: { maximum_health: 100 } },
};

test("canonical differences include only values that differ from server defaults", () => {
  assert.deepEqual(canonicalDifferences([field], baseline, baseline), []);
  const draft = { fighterProfiles: { default: { maximum_health: 125 } } };
  assert.deepEqual(canonicalDifferences([field], draft, baseline), [
    { field, serverDefault: 100, value: 125 },
  ]);
});

test("copied canonical differences are readable and retain their exact paths", () => {
  const draft = { fighterProfiles: { default: { maximum_health: 125 } } };
  const text = formatCanonicalDifferences(canonicalDifferences([field], draft, baseline));
  assert.match(text, /^Balance Lab changes from server defaults/);
  assert.match(text, /Fighters \/ Default \/ Core stats \/ Maximum health/);
  assert.match(text, /\/fighterProfiles\/default\/maximum_health/);
  assert.match(text, /100 -> 125 health/);
});
