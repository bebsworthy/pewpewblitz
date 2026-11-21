import { useEffect, useState } from "react";
import {
  displayNumber,
  formatFieldNumber,
  pathKey,
  storedNumber,
} from "../lib/editorFields";
import type {
  BalanceLabSnapshot,
  EditorFieldDescriptor,
} from "../model/balanceLab";

interface Props {
  field: EditorFieldDescriptor;
  subjectLabel: string;
  draft: BalanceLabSnapshot;
  applied: BalanceLabSnapshot;
  baseline: BalanceLabSnapshot;
  error?: string;
  disabled: boolean;
  selected: boolean;
  onSelect: (field: EditorFieldDescriptor) => void;
  onChange: (field: EditorFieldDescriptor, value: string) => void;
}

export function EditorMatrixCell({
  field,
  subjectLabel,
  draft,
  applied,
  baseline,
  error,
  disabled,
  selected,
  onSelect,
  onChange,
}: Props) {
  const display = displayNumber(draft, field);
  const changed = storedNumber(draft, field) !== storedNumber(applied, field);
  const differsFromDefault = storedNumber(draft, field) !== storedNumber(baseline, field);
  const [text, setText] = useState(formatFieldNumber(display, field));
  const id = pathKey(field.path);
  const selectedLabel = field.options?.find((option) => option.value === display)?.label;

  useEffect(
    () => setText(formatFieldNumber(display, field)),
    [display, field.storageKind, field.storageScale, field.unit],
  );

  const update = (next: string) => {
    setText(next);
    onChange(field, next);
  };

  return (
    <td
      className={`matrix-cell ${changed ? "changed" : ""} ${differsFromDefault ? "non-default" : ""} ${error ? "invalid" : ""} ${selected ? "selected" : ""}`}
    >
      <div className="matrix-input-wrap">
        {field.control === "select" ? (
          <select
            id={id}
            aria-label={`${field.label}, ${subjectLabel}`}
            aria-describedby={error ? `${id}-error` : undefined}
            aria-invalid={Boolean(error)}
            value={String(display)}
            title={selectedLabel}
            disabled={disabled}
            onFocus={() => onSelect(field)}
            onChange={(event) => update(event.target.value)}
          >
            {field.options?.map((option) => (
              <option value={option.value} key={option.value}>{option.label}</option>
            ))}
          </select>
        ) : (
        <input
          id={id}
          aria-label={`${field.label}, ${subjectLabel}`}
          aria-describedby={error ? `${id}-error` : undefined}
          aria-invalid={Boolean(error)}
          type="number"
          min={field.minimumExclusive ? undefined : field.minimum}
          max={field.maximum}
          step={field.step}
          value={text}
          disabled={disabled}
          onFocus={() => onSelect(field)}
          onChange={(event) => update(event.target.value)}
        />
        )}
        <span className="matrix-cell-markers">
          {changed && <span className="changed-marker" aria-label="Edited since apply" />}
          {differsFromDefault && <span className="default-marker" aria-label="Differs from server default" />}
        </span>
      </div>
      {error && (
        <span className="matrix-cell-error" id={`${id}-error`} role="alert">
          {error}
        </span>
      )}
    </td>
  );
}
