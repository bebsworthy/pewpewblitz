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
  draft: BalanceLabSnapshot;
  applied: BalanceLabSnapshot;
  baseline: BalanceLabSnapshot;
  error?: string;
  disabled: boolean;
  onChange: (field: EditorFieldDescriptor, value: string) => void;
  onReset: (field: EditorFieldDescriptor) => void;
}

export function EditorFieldRow({
  field,
  draft,
  applied,
  baseline,
  error,
  disabled,
  onChange,
  onReset,
}: Props) {
  const display = displayNumber(draft, field);
  const appliedDisplay = displayNumber(applied, field);
  const baselineDisplay = displayNumber(baseline, field);
  const changed = storedNumber(draft, field) !== storedNumber(applied, field);
  const differsFromDefault = storedNumber(draft, field) !== storedNumber(baseline, field);
  const [text, setText] = useState(formatFieldNumber(display, field));

  useEffect(
    () => setText(formatFieldNumber(display, field)),
    [display, field.storageKind, field.storageScale, field.unit],
  );

  const update = (next: string) => {
    setText(next);
    onChange(field, next);
  };

  return (
    <div className={`field-row ${changed ? "changed" : ""} ${differsFromDefault ? "non-default" : ""} ${error ? "invalid" : ""}`}>
      <div className="field-copy">
        <div className="field-title">
          <label htmlFor={pathKey(field.path)}>{field.label}</label>
          {changed && <span className="changed-badge">Changed</span>}
          {differsFromDefault && <span className="default-difference-badge">Non-default</span>}
        </div>
        <p>
          Applied {formatFieldNumber(appliedDisplay, field)} · Default {formatFieldNumber(baselineDisplay, field)} {field.unit}
        </p>
        {field.help && <small>{field.help}</small>}
      </div>

      <div className={`field-control ${field.control}`}>
        {field.control === "range-and-number" && (
          <input
            aria-label={`${field.label} slider`}
            type="range"
            min={field.minimum}
            max={field.maximum}
            step={field.step}
            value={display}
            disabled={disabled}
            onChange={(event) => update(event.target.value)}
          />
        )}
        <div className="exact-value">
          <input
            id={pathKey(field.path)}
            aria-describedby={error ? `${pathKey(field.path)}-error` : undefined}
            aria-invalid={Boolean(error)}
            type="number"
            min={field.minimumExclusive ? undefined : field.minimum}
            max={field.maximum}
            step={field.step}
            value={text}
            disabled={disabled}
            onChange={(event) => update(event.target.value)}
          />
          <span>{field.unit}</span>
        </div>
        <button
          className="field-reset"
          type="button"
          disabled={disabled || !changed}
          onClick={() => onReset(field)}
        >
          Reset
        </button>
      </div>
      {error && (
        <p className="field-error" id={`${pathKey(field.path)}-error`} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
