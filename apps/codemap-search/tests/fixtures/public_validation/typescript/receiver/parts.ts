// The only indexed `formatToParts`, a module-level function.
export function formatToParts(value: number): string[] {
  return String(value).split('');
}

// The receiver is declared with a type that this repository does not declare.
export function dateParts(formatter: Intl.DateTimeFormat, date: Date): Intl.DateTimeFormatPart[] {
  return formatter.formatToParts(date);
}

// The receiver type is not written here, so this call stays unconfirmed.
export function anyParts(formatter: any, date: Date): unknown {
  return formatter.formatToParts(date);
}
