import assert from "node:assert/strict";
import test from "node:test";
import { buildEditorMatrix } from "../src/pages/balance-lab/lib/editorMatrix.ts";

const field = (overrides = {}) => ({
  path: ["fighterProfiles", "default", "maximum_health"],
  section: "fighters",
  comparisonKey: "maximum_health",
  subjectKey: "default",
  subjectLabel: "Default",
  group: "Core stats",
  label: "Maximum health",
  storageKind: "integer",
  unit: "health",
  storageScale: 1,
  minimum: 1,
  maximum: 65_535,
  minimumExclusive: false,
  step: 1,
  control: "number",
  ...overrides,
});

test("builds stable subject columns and aligned rows in manifest order", () => {
  const fields = [
    field(),
    field({
      path: ["fighterProfiles", "default", "movement_speed"],
      comparisonKey: "movement_speed",
      label: "Movement speed",
      unit: "world units/s",
    }),
    field({
      path: ["fighterProfiles", "lightweight", "maximum_health"],
      subjectKey: "lightweight",
      subjectLabel: "Lightweight",
    }),
    field({
      path: ["fighterProfiles", "lightweight", "movement_speed"],
      comparisonKey: "movement_speed",
      subjectKey: "lightweight",
      subjectLabel: "Lightweight",
      label: "Movement speed",
      unit: "world units/s",
    }),
  ];
  const matrix = buildEditorMatrix(fields, "fighters");
  assert.deepEqual(matrix.subjects.map(({ key }) => key), ["default", "lightweight"]);
  assert.deepEqual(matrix.groups[0].rows.map(({ key }) => key), [
    "maximum_health",
    "movement_speed",
  ]);
  assert.equal(matrix.groups[0].rows[0].cells.get("lightweight"), fields[2]);
  assert.equal(matrix.fieldCount, fields.length);
});

test("preserves sparse subject and row intersections", () => {
  const fields = [
    field({ section: "weapons", subjectKey: "pulse", subjectLabel: "Pulse" }),
    field({
      path: ["weapons", 1, "spread"],
      section: "weapons",
      subjectKey: "scatter",
      subjectLabel: "Scatter",
      comparisonKey: "spread",
      label: "Spread angle",
      unit: "degrees",
    }),
  ];
  const matrix = buildEditorMatrix(fields, "weapons");
  assert.equal(matrix.groups[0].rows[0].cells.has("scatter"), false);
  assert.equal(matrix.groups[0].rows[1].cells.has("pulse"), false);
  assert.equal(matrix.fieldCount, 2);
});

test("rejects duplicate subject and comparison row cells", () => {
  assert.throws(
    () => buildEditorMatrix([field(), field({ path: ["duplicate"] })], "fighters"),
    /Duplicate comparison cell/,
  );
});

test("rejects incompatible fields sharing a comparison row", () => {
  assert.throws(
    () => buildEditorMatrix([
      field(),
      field({
        subjectKey: "lightweight",
        subjectLabel: "Lightweight",
        unit: "seconds",
      }),
    ], "fighters"),
    /Incompatible comparison row/,
  );
});
