export function requiredArgValue(args: string[], index: number, flag: string): string {
  const value = args[index + 1];
  return requiredValue(value, flag);
}

export function requiredValue(value: string | undefined, name: string): string {
  if (!value || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

export function nonEmpty(value: string, field: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${field} must not be empty`);
  }
  return trimmed;
}
