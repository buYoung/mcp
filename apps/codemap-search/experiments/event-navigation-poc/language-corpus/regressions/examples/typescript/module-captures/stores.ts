export const left = new Map<string, () => void>();
export const right = new Map<string, () => void>();
export function makeStore() {
  const values = new Map<string, () => void>();
  return {
    put(name: string, callback: () => void) {
      values.set(name, callback); // @S2
    },
    get(name: string) { return values.get(name); },
  };
}
