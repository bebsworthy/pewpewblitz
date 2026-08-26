import { useEffect, useMemo, useState } from "react";
import { pathKey } from "../lib/editorFields";
import type {
  BalanceLabSnapshot,
  EditorFieldDescriptor,
  EditorSection,
} from "../model/balanceLab";
import { EditorFieldRow } from "./EditorFieldRow";

const sections: { key: EditorSection; label: string }[] = [
  { key: "fighters", label: "Fighters" },
  { key: "weapons", label: "Weapons" },
  { key: "ultimates", label: "Ultimates" },
  { key: "world-objects", label: "World objects" },
  { key: "modes", label: "Modes" },
];

const sectionHelp: Record<EditorSection, string> = {
  fighters: "Creation profiles shared by saved brawlers.",
  weapons: "Authoritative weapon economy, delivery, payload, and world effects.",
  ultimates: "Concealment and reveal timing, targeting, and area rules.",
  "world-objects": "Durability, explosions, and restoration pickups.",
  modes: "Mode-owned objective rules.",
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
  const sectionFields = useMemo(
    () => props.fields.filter((field) => field.section === section),
    [props.fields, section],
  );
  const subjects = useMemo(
    () => Array.from(new Map(sectionFields.map((field) => [field.subjectKey, field.subjectLabel]))),
    [sectionFields],
  );
  const [selectedSubjects, setSelectedSubjects] = useState<Partial<Record<EditorSection, string>>>({});
  const errorField = props.fields.find((field) => props.errors[pathKey(field.path)]);
  const errorKey = errorField ? pathKey(errorField.path) : null;

  useEffect(() => {
    if (!errorField || !errorKey) return;
    setSection(errorField.section);
    setSelectedSubjects((current) => ({
      ...current,
      [errorField.section]: errorField.subjectKey,
    }));
    window.requestAnimationFrame(() => document.getElementById(errorKey)?.focus());
  }, [errorKey]);

  const subject = selectedSubjects[section] && subjects.some(([key]) => key === selectedSubjects[section])
    ? selectedSubjects[section]!
    : subjects[0]?.[0];
  const visibleFields = sectionFields.filter((field) => field.subjectKey === subject);
  const groups = Array.from(new Set(visibleFields.map((field) => field.group)));
  const subjectLabel = subjects.find(([key]) => key === subject)?.[1] ?? "No editable fields";

  return (
    <>
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

      <div className="workspace-layout">
        <aside className="subject-nav" aria-label={`${section} entries`}>
          <p>{sections.find((item) => item.key === section)?.label}</p>
          {subjects.map(([key, label]) => (
            <button
              type="button"
              className={subject === key ? "active" : ""}
              onClick={() => setSelectedSubjects((current) => ({ ...current, [section]: key }))}
              key={key}
            >
              {label}
            </button>
          ))}
        </aside>

        <section className="editor-panel">
          <header className="editor-heading">
            <div>
              <p className="eyebrow">{sections.find((item) => item.key === section)?.label}</p>
              <h2>{subjectLabel}</h2>
              <p>{sectionHelp[section]}</p>
            </div>
            <span>
              {visibleFields.length} editable {visibleFields.length === 1 ? "field" : "fields"}
            </span>
          </header>

          {groups.map((group) => (
            <section className="field-group" key={group}>
              <h3>{group}</h3>
              {visibleFields.filter((field) => field.group === group).map((field) => (
                <EditorFieldRow
                  {...props}
                  field={field}
                  error={props.errors[pathKey(field.path)]}
                  key={pathKey(field.path)}
                />
              ))}
            </section>
          ))}
        </section>
      </div>
    </>
  );
}
