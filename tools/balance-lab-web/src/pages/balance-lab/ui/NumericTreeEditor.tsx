import type { JsonValue } from "../model/balanceLab";
import { constraintHint, fieldLabel, numberSpec, secondsFor } from "../lib/fields";

interface Props {
  value: JsonValue;
  path: (string | number)[];
  onNumber: (path: (string | number)[], value: number) => void;
}

const readOnlyNumber = (path: (string | number)[]) => {
  const key = String(path.at(-1));
  return key === "id" || key === "schemaVersion";
};

export function NumericTreeEditor({ value, path, onNumber }: Props) {
  if (typeof value === "number") {
    const key = String(path.at(-1));
    if (readOnlyNumber(path)) return <span className="readonly-value">{value}</span>;
    const spec = numberSpec(key, value, path);
    const seconds = secondsFor(key, value);
    const constraint = constraintHint(path);
    return (
      <div className="number-control">
        <input
          aria-label={fieldLabel(key)}
          type="range"
          value={value}
          {...spec}
          onChange={(event) => onNumber(path, Number(event.target.value))}
        />
        <input
          aria-label={`${fieldLabel(key)} exact value`}
          type="number"
          value={value}
          {...spec}
          onChange={(event) => onNumber(path, Number(event.target.value))}
        />
        {seconds && <small>{seconds}</small>}
        {constraint && <small>{constraint}</small>}
      </div>
    );
  }
  if (typeof value === "string" || typeof value === "boolean" || value === null) {
    return <span className="readonly-value">{String(value)}</span>;
  }
  const entries = Array.isArray(value)
    ? value.map((entry, index) => [index, entry] as const)
    : Object.entries(value);
  return (
    <div className="tree">
      {entries.map(([key, entry]) => {
        const nextPath = [...path, key];
        const nested = typeof entry === "object" && entry !== null;
        return (
          <section className={nested ? "tree-group" : "tree-row"} key={String(key)}>
            <div className="tree-label">{fieldLabel(String(key))}</div>
            <NumericTreeEditor value={entry} path={nextPath} onNumber={onNumber} />
          </section>
        );
      })}
    </div>
  );
}
