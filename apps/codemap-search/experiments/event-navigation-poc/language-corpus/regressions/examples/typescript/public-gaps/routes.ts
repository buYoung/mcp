type Hook = { pre: () => void };
class Router {
  private values = new Map<string, Record<string, Hook>>();
  private other = new Map<string, Record<string, Hook>>();
  install(key: string, hooks: Record<string, Hook>) {
    this.values.set(key, { ...hooks }); // @S1
    this.other.set(key, { ...hooks }); // @S2
  }
  fire(key: string, name: string) {
    this.values.get(key)?.[name].pre(); // @I1
  }
  known(callback: () => void) {
    const input = { allowed: { pre: callback } };
    this.values.set('known', { ...input }); // @S3
  }
  fireKnown() { this.values.get('known')?.allowed.pre(); } // @I2
  fireMissing() { this.values.get('known')?.missing.pre(); } // @I3
}
