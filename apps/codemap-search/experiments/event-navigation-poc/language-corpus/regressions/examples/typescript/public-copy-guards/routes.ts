class Router {
  values = new Map<string, any>();
  install(hooks: any) {
    this.values.set('override', { // @S1
      ...hooks,
      blocked: {},
    });
  }
  dynamic(name: string) { this.values.get('override')[name].pre(); } // @I1
  blocked() { this.values.get('override').blocked.pre(); } // @I2
  known(callback: () => void) {
    const input: any = { allowed: { pre: callback }, empty: {} };
    input.added = { pre: callback };
    this.values.set('known', { ...input }); // @S2
  }
  added() { this.values.get('known').added.pre(); } // @I3
  empty() { this.values.get('known').empty.pre(); } // @I4
  unknown() { this.values.get('known').absent.pre(); } // @I5
  overwrite(callback: () => void) {
    const input = { allowed: { pre: callback } };
    const replacement = { allowed: {} };
    this.values.set('later', { ...input, ...replacement }); // @S3
  }
  later() { this.values.get('later').allowed.pre(); } // @I6
}
