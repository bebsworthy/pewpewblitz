import { useEffect, useMemo, useState } from "react";
import { pathKey } from "../lib/editorFields";
import { buildEditorMatrix } from "../lib/editorMatrix";
import type {
  BalanceLabSnapshot,
  EditorFieldDescriptor,
  EditorSection,
} from "../model/balanceLab";
import { EditorFieldDetail } from "./EditorFieldDetail";
import { EditorMatrix } from "./EditorMatrix";

const sections: { key: EditorSection; label: string }[] = [
  { key: "global", label: "Global" },
  { key: "fighters", label: "Fighters" },
  { key: "weapons", label: "Weapons" },
  { key: "ultimates", label: "Ultimates" },
  { key: "passives", label: "Passives" },
  { key: "world-objects", label: "World objects" },
  { key: "modes", label: "Modes" },
];

const sectionHelp: Record<EditorSection, string> = {
  global: "Match-wide authoritative gameplay rules shared by every fighter.",
  fighters: "Compare the creation profiles available to saved brawlers.",
  weapons: "Compare authoritative weapon economy, delivery, payload, and world effects.",
  ultimates: "Compare targeting, timing, deployables, and area rules.",
  passives: "Compare the tunable effects and timings of equipped passive abilities.",
  "world-objects": "Compare map effects, durability, explosions, and restoration pickups.",
  modes: "Compare mode-owned objective rules.",
};

interface Props {
  fields: EditorFieldDescriptor[];
  draft: BalanceLabSnapshot;
  applied: BalanceLabSnapshot;
  baseline: BalanceLabSnapshot;
  errors: Record<string, string>;
  disabled: boolean;
  onChange: (field: EditorFieldDescriptor, value: string) => void;
  onReset: (field: EditorFieldDescriptor) => void;
}

export function EditorWorkspace(props: Props) {
  const [section, setSection] = useState<EditorSection>("fighters");
  const [focusedFields, setFocusedFields] = useState<Partial<Record<EditorSection, string>>>({});
  const matrix = useMemo(
    () => buildEditorMatrix(props.fields, section),
    [props.fields, section],
  );
  const sectionFields = useMemo(
    () => props.fields.filter((field) => field.section === section),
    [props.fields, section],
  );
  const errorField = props.fields.find((field) => props.errors[pathKey(field.path)]);
  const errorKey = errorField ? pathKey(errorField.path) : null;

  useEffect(() => {
    if (!errorField || !errorKey) return;
    setSection(errorField.section);
    setFocusedFields((current) => ({
      ...current,
      [errorField.section]: errorKey,
    }));
    window.requestAnimationFrame(() => document.getElementById(errorKey)?.focus());
  }, [errorKey]);

  const focusedKey = focusedFields[section];
  const focusedField = sectionFields.find((field) => pathKey(field.path) === focusedKey)
    ?? sectionFields[0];
  const sectionLabel = sections.find((item) => item.key === section)?.label ?? section;

  return (
    <section className="editor-workspace" aria-labelledby="editor-heading">
      <nav className="section-nav" aria-label="Balance categories">
        {sections.map((item) => (
          <button
            type="button"
            className={section === item.key ? "active" : ""}
            aria-current={section === item.key ? "page" : undefined}
            onClick={() => setSection(item.key)}
            key={item.key}
          >
            {item.label}
          </button>
        ))}
      </nav>

      <div className="editor-panel">
        <header className="editor-heading">
          <div>
            <p className="eyebrow">Comparison workspace</p>
            <h2 id="editor-heading">{sectionLabel}</h2>
            <p>{sectionHelp[section]}</p>
          </div>
          <div className="matrix-summary">
            <span>{matrix.subjects.length} {matrix.subjects.length === 1 ? "column" : "columns"}</span>
            <span>{matrix.fieldCount} editable {matrix.fieldCount === 1 ? "field" : "fields"}</span>
          </div>
        </header>

        <div className="matrix-legend" aria-label="Value state legend">
          <span><i className="legend-edited" /> Edited since apply</span>
          <span><i className="legend-default" /> Differs from server default</span>
          <span>Focus a value for bounds, help, slider, and reset.</span>
        </div>

        <EditorMatrix
          {...props}
          matrix={matrix}
          selectedField={focusedField}
          onSelect={(field) => setFocusedFields((current) => ({
            ...current,
            [section]: pathKey(field.path),
          }))}
        />

        <EditorFieldDetail
          {...props}
          field={focusedField}
          error={focusedField ? props.errors[pathKey(focusedField.path)] : undefined}
        />
      </div>
    </section>
  );
}
