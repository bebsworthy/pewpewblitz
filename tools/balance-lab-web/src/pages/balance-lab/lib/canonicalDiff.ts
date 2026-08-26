import {
  displayNumber,
  formatNumber,
  pointerPath,
  storedNumber,
} from "./editorFields.ts";
import type {
  BalanceLabSnapshot,
  EditorFieldDescriptor,
  EditorSection,
} from "../model/balanceLab";

export interface CanonicalDifference {
  field: EditorFieldDescriptor;
  value: number;
  serverDefault: number;
}

const sectionLabels: Record<EditorSection, string> = {
  fighters: "Fighters",
  weapons: "Weapons",
  ultimates: "Ultimates",
  "world-objects": "World objects",
  modes: "Modes",
};

export function canonicalDifferences(
  fields: EditorFieldDescriptor[],
  snapshot: BalanceLabSnapshot,
  baseline: BalanceLabSnapshot,
): CanonicalDifference[] {
  return fields
    .filter((field) => storedNumber(snapshot, field) !== storedNumber(baseline, field))
    .map((field) => ({
      field,
      value: displayNumber(snapshot, field),
      serverDefault: displayNumber(baseline, field),
    }));
}

export function formatCanonicalDifferences(differences: CanonicalDifference[]) {
  const lines = differences.map(({ field, value, serverDefault }) => {
    const context = [sectionLabels[field.section], field.subjectLabel, field.group, field.label]
      .filter((part, index, all) => index === 0 || part !== all[index - 1])
      .join(" / ");
    return `- ${context} (${pointerPath(field.path)}): ${formatNumber(serverDefault)} -> ${formatNumber(value)} ${field.unit}`;
  });
  return ["Balance Lab changes from server defaults", "", ...lines].join("\n");
}
