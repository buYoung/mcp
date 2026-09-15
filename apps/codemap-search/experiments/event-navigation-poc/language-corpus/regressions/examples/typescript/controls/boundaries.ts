class Router {
  constructor(
    private primary: () => void, // @S1
    private secondary: () => void, // @S2
  ) {}
  get callback(): () => void { return this.primary; }
  set callback(f: () => void) { this.secondary = f; }
  get other(): () => void { return this.secondary; }
  fire() {
    const fn = this.callback;
    fn(); // @I1
  }
  fireOther() {
    const fn = this.other;
    fn(); // @I2
  }
  change(f: () => void) { this.callback = f; } // @S3
}
