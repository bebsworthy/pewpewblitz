import type {
  EditorFieldDescriptor,
  EditorSection,
} from "../model/balanceLab";

export interface EditorMatrixSubject {
  key: string;
  label: string;
}

export interface EditorMatrixRow {
  key: string;
  group: string;
  label: string;
  unit: string;
  cells: Map<string, EditorFieldDescriptor>;
}

export interface EditorMatrixGroup {
  label: string;
  rows: EditorMatrixRow[];
}

export interface EditorMatrix {
  subjects: EditorMatrixSubject[];
  groups: EditorMatrixGroup[];
  fieldCount: number;
}

function assertCompatibleRow(
  row: EditorMatrixRow,
  field: EditorFieldDescriptor,
) {
  if (
    row.group !== field.group
    || row.label !== field.label
    || row.unit !== field.unit
  ) {
    throw new Error(
      `Incompatible comparison row ${field.section}/${field.comparisonKey}`,
    );
  }
}

export function buildEditorMatrix(
  fields: EditorFieldDescriptor[],
  section: EditorSection,
): EditorMatrix {
  const sectionFields = fields.filter((field) => field.section === section);
  const subjects = new Map<string, EditorMatrixSubject>();
  const rows = new Map<string, EditorMatrixRow>();
  const groups = new Map<string, EditorMatrixGroup>();

  for (const field of sectionFields) {
    if (!field.comparisonKey) {
      throw new Error(`Missing comparison key for ${JSON.stringify(field.path)}`);
    }
    const subject = subjects.get(field.subjectKey);
    if (subject && subject.label !== field.subjectLabel) {
      throw new Error(`Inconsistent subject label for ${field.subjectKey}`);
    }
    subjects.set(field.subjectKey, {
      key: field.subjectKey,
      label: field.subjectLabel,
    });

    let row = rows.get(field.comparisonKey);
    if (!row) {
      row = {
        key: field.comparisonKey,
        group: field.group,
        label: field.label,
        unit: field.unit,
        cells: new Map(),
      };
      rows.set(field.comparisonKey, row);
      let group = groups.get(field.group);
      if (!group) {
        group = { label: field.group, rows: [] };
        groups.set(field.group, group);
      }
      group.rows.push(row);
    } else {
      assertCompatibleRow(row, field);
    }

    if (row.cells.has(field.subjectKey)) {
      throw new Error(
        `Duplicate comparison cell ${section}/${field.subjectKey}/${field.comparisonKey}`,
      );
    }
    row.cells.set(field.subjectKey, field);
  }

  const fieldCount = Array.from(rows.values()).reduce(
    (count, row) => count + row.cells.size,
    0,
  );
  if (fieldCount !== sectionFields.length) {
    throw new Error(`Comparison matrix lost fields in ${section}`);
  }

  return {
    subjects: Array.from(subjects.values()),
    groups: Array.from(groups.values()),
    fieldCount,
  };
}
