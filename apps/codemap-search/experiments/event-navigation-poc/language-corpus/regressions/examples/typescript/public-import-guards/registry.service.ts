export class Registry {
  values = new Map<string, () => void>();
  install(name: string, callback: () => void) {
    this.values.set(name, callback); // @S1
  }
  get(name: string) { return this.values.get(name); }
}
