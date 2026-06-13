import { readFileSync } from "node:fs";

export type JsonRecord = Record<string, unknown>;

export function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function readJsonRecord(
  path: string,
  label: string,
  expectation = "must contain a JSON object",
): JsonRecord {
  const parsed = JSON.parse(readFileSync(path, "utf8")) as unknown;
  if (!isRecord(parsed)) {
    throw new Error(`${label} ${expectation}`);
  }
  return parsed;
}

export function stringField(record: JsonRecord, field: string): string {
  const value = record[field];
  if (typeof value !== "string") {
    throw new Error(`${field} must be a string`);
  }
  return value;
}

export function optionalStringField(record: JsonRecord, field: string): string | undefined {
  const value = record[field];
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "string") {
    throw new Error(`${field} must be a string when present`);
  }
  return value;
}

export function finiteNumberField(record: JsonRecord, field: string): number {
  const value = record[field];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${field} must be a finite number`);
  }
  return value;
}

export function optionalFiniteNumberField(record: JsonRecord, field: string): number | undefined {
  const value = record[field];
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${field} must be a finite number when present`);
  }
  return value;
}

export function stringArrayField(record: JsonRecord, field: string): string[] {
  const value = record[field];
  if (!Array.isArray(value) || !value.every((entry) => typeof entry === "string")) {
    throw new Error(`${field} must be a string array`);
  }
  return value;
}
