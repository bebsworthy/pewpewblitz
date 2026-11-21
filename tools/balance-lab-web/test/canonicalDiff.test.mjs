import assert from "node:assert/strict";
import test from "node:test";
import {
  canonicalDifferences,
  formatCanonicalDifferences,
} from "../src/pages/balance-lab/lib/canonicalDiff.ts";

const field = {
  path: ["heist", "safe_maximum_health"],
  section: "world-objects",
  subjectKey: "safe",
  subjectLabel: "Safe",
  group: "Durability",
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
  heist: { safe_maximum_health: 100 },
};

test("canonical differences include only values that differ from server defaults", () => {
  assert.deepEqual(canonicalDifferences([field], baseline, baseline), []);
  const draft = { heist: { safe_maximum_health: 125 } };
  assert.deepEqual(canonicalDifferences([field], draft, baseline), [
    { field, serverDefault: 100, value: 125 },
  ]);
});

test("copied canonical differences are readable and retain their exact paths", () => {
  const draft = { heist: { safe_maximum_health: 125 } };
  const text = formatCanonicalDifferences(canonicalDifferences([field], draft, baseline));
  assert.match(text, /^Balance Lab changes from server defaults/);
  assert.match(text, /World objects \/ Safe \/ Durability \/ Maximum health/);
  assert.match(text, /\/heist\/safe_maximum_health/);
  assert.match(text, /100 -> 125 health/);
});

test("copied choice differences name the chosen options", () => {
  const choice = {
    ...field,
    path: ["characters", 0, "ultimateOptions", 1],
    section: "characters",
    subjectLabel: "Riff",
    group: "Options",
    label: "Ultimate option 2",
    control: "select",
    unit: "",
    options: [
      { value: 3, label: "Reveal Scan" },
      { value: 11, label: "Big Blob" },
    ],
  };
  const before = { characters: [{ ultimateOptions: [1, 3] }] };
  const after = { characters: [{ ultimateOptions: [1, 11] }] };
  const text = formatCanonicalDifferences(canonicalDifferences([choice], after, before));
  assert.match(text, /Characters \/ Riff \/ Options \/ Ultimate option 2/);
  assert.match(text, /Reveal Scan -> Big Blob$/m);
});
