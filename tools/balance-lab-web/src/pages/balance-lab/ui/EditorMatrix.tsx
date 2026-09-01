import { pathKey } from "../lib/editorFields";
import type { EditorMatrix as Matrix } from "../lib/editorMatrix";
import type {
  BalanceLabSnapshot,
  EditorFieldDescriptor,
} from "../model/balanceLab";
import { EditorMatrixCell } from "./EditorMatrixCell";

interface Props {
  matrix: Matrix;
  draft: BalanceLabSnapshot;
  applied: BalanceLabSnapshot;
  baseline: BalanceLabSnapshot;
  errors: Record<string, string>;
  disabled: boolean;
  selectedField?: EditorFieldDescriptor;
  onSelect: (field: EditorFieldDescriptor) => void;
  onChange: (field: EditorFieldDescriptor, value: string) => void;
}

export function EditorMatrix(props: Props) {
  const columnCount = props.matrix.subjects.length + 1;
  return (
    <div className="matrix-scroll" tabIndex={0} aria-label="Editable comparison matrix">
      <table className="comparison-matrix">
        <thead>
          <tr>
            <th className="property-column" scope="col">Property</th>
            {props.matrix.subjects.map((subject) => (
              <th scope="col" key={subject.key}>{subject.label}</th>
            ))}
          </tr>
        </thead>
        {props.matrix.groups.map((group) => (
          <tbody key={group.label}>
            <tr className="matrix-group-row">
              <th colSpan={columnCount} scope="rowgroup">{group.label}</th>
            </tr>
            {group.rows.map((row) => (
              <tr key={row.key}>
                <th className="matrix-row-heading" scope="row">
                  <span>{row.label}</span>
                  {row.unit && <small>{row.unit}</small>}
                </th>
                {props.matrix.subjects.map((subject) => {
                  const field = row.cells.get(subject.key);
                  return field ? (
                    <EditorMatrixCell
                      {...props}
                      field={field}
                      subjectLabel={subject.label}
                      error={props.errors[pathKey(field.path)]}
                      selected={pathKey(field.path) === (props.selectedField ? pathKey(props.selectedField.path) : null)}
                      key={subject.key}
                    />
                  ) : (
                    <td
                      className="matrix-cell unavailable"
                      aria-label={`${row.label} is not available for ${subject.label}`}
                      key={subject.key}
                    >
                      —
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        ))}
      </table>
    </div>
  );
}
