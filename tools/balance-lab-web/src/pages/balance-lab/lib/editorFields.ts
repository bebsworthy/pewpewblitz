import type {
  BalanceLabSnapshot,
  EditorFieldDescriptor,
  EditorPath,
  JsonValue,
} from "../model/balanceLab";

export function pathKey(path: EditorPath) {
  return JSON.stringify(path);
}

export function pointerPath(path: EditorPath) {
  return `/${path.map(String).join("/")}`;
}

export function fieldFromServerError(
  message: string,
  fields: EditorFieldDescriptor[],
) {
  const match = /^field ([^:]+):\s*(.+)$/.exec(message);
  if (!match) return null;
  const field = fields.find((candidate) => pointerPath(candidate.path) === match[1]);
  return field ? { field, message: match[2] } : null;
}

export function readAtPath(value: JsonValue, path: EditorPath): JsonValue {
  return path.reduce<JsonValue>((current, segment) => {
    if (Array.isArray(current)) return current[Number(segment)];
    return (current as Record<string, JsonValue>)[String(segment)];
  }, value);
}

export function replaceAtPath(
  value: JsonValue,
  path: EditorPath,
  next: number,
): JsonValue {
  if (path.length === 0) return next;
  const [head, ...tail] = path;
  if (Array.isArray(value)) {
    const copy = [...value];
    copy[Number(head)] = replaceAtPath(copy[Number(head)], tail, next);
    return copy;
  }
  const copy = { ...(value as Record<string, JsonValue>) };
  copy[String(head)] = replaceAtPath(copy[String(head)], tail, next);
  return copy;
}

export function storedNumber(snapshot: BalanceLabSnapshot, field: EditorFieldDescriptor) {
  const value = readAtPath(snapshot, field.path);
  if (typeof value !== "number") {
    throw new Error(`Editor path ${pathKey(field.path)} is not numeric`);
  }
  return value;
}

export function displayNumber(snapshot: BalanceLabSnapshot, field: EditorFieldDescriptor) {
  return storedNumber(snapshot, field) / field.storageScale;
}

export function toStoredNumber(display: number, field: EditorFieldDescriptor) {
  const scaled = display * field.storageScale;
  return field.storageKind === "integer" ? Math.round(scaled) : scaled;
}

export function validateDisplayNumber(display: number, field: EditorFieldDescriptor) {
  if (!Number.isFinite(display)) return "Enter a number.";
  if (field.minimumExclusive ? display <= field.minimum : display < field.minimum) {
    const relation = field.minimumExclusive ? "greater than" : "at least";
    return `Must be ${relation} ${formatNumber(field.minimum)} ${field.unit}.`;
  }
  if (display > field.maximum) {
    return `Must be at most ${formatNumber(field.maximum)} ${field.unit}.`;
  }
  const stored = display * field.storageScale;
  if (
    field.storageKind === "integer" &&
    field.storageScale === 1 &&
    Math.abs(stored - Math.round(stored)) > 0.0001
  ) {
    return `Must align to increments of ${formatNumber(field.step)} ${field.unit}.`;
  }
  return null;
}

export function changedFields(
  fields: EditorFieldDescriptor[],
  draft: BalanceLabSnapshot,
  applied: BalanceLabSnapshot,
) {
  return fields.filter((field) => storedNumber(draft, field) !== storedNumber(applied, field));
}

export function formatNumber(value: number) {
  return Number.isInteger(value) ? String(value) : String(Number(value.toFixed(6)));
}

export function formatFieldNumber(value: number, field: EditorFieldDescriptor) {
  if (field.storageKind === "integer" && field.storageScale === 60 && field.unit === "s") {
    return String(Number(value.toFixed(2)));
  }
  return formatNumber(value);
}
