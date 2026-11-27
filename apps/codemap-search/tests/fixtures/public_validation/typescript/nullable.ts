export class Gauge {
  level(): number {
    return 1;
  }
}

export class Dial {
  level(): number {
    return 2;
  }
}

// The receivers are declared as a union with null or undefined.
export function safe(gauge: Gauge | null): number {
  return gauge?.level() ?? 0;
}

export function asserted(gauge: Gauge | undefined): number {
  return gauge!.level();
}

// A union of two types does not say which member is called.
export function either(gauge: Gauge | Dial): number {
  return gauge.level();
}
