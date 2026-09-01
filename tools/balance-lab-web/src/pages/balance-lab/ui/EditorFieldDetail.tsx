import {
  displayNumber,
  formatFieldNumber,
  storedNumber,
} from "../lib/editorFields";
import type {
  BalanceLabSnapshot,
  EditorFieldDescriptor,
} from "../model/balanceLab";

interface Props {
  field?: EditorFieldDescriptor;
  draft: BalanceLabSnapshot;
  applied: BalanceLabSnapshot;
  baseline: BalanceLabSnapshot;
  error?: string;
  disabled: boolean;
  onChange: (field: EditorFieldDescriptor, value: string) => void;
  onReset: (field: EditorFieldDescriptor) => void;
}

export function EditorFieldDetail({
  field,
  draft,
  applied,
  baseline,
  error,
  disabled,
  onChange,
  onReset,
}: Props) {
  if (!field) return null;
  const display = displayNumber(draft, field);
  const changed = storedNumber(draft, field) !== storedNumber(applied, field);
  const rangeStart = `${field.minimumExclusive ? "> " : ""}${formatFieldNumber(field.minimum, field)}`;

  return (
    <aside className="field-detail" aria-live="polite" aria-label="Focused field details">
      <div className="field-detail-copy">
        <p className="eyebrow">{field.subjectLabel} · {field.group}</p>
        <h3>{field.label}</h3>
        <p>
          Applied <strong>{formatFieldNumber(displayNumber(applied, field), field)}</strong>
          <span aria-hidden="true"> · </span>
          Default <strong>{formatFieldNumber(displayNumber(baseline, field), field)}</strong>
          <span aria-hidden="true"> · </span>
          Range <strong>{rangeStart}–{formatFieldNumber(field.maximum, field)} {field.unit}</strong>
        </p>
        {field.help && <small>{field.help}</small>}
        {error && <small className="field-detail-error">{error}</small>}
      </div>
      <div className="field-detail-control">
        {field.control === "range-and-number" && (
          <input
            aria-label={`${field.label}, ${field.subjectLabel} slider`}
            type="range"
            min={field.minimum}
            max={field.maximum}
            step={field.step}
            value={display}
            disabled={disabled}
            onChange={(event) => onChange(field, event.target.value)}
          />
        )}
        <button
          type="button"
          disabled={disabled || !changed}
          onClick={() => onReset(field)}
        >
          Reset to applied
        </button>
      </div>
    </aside>
  );
}
