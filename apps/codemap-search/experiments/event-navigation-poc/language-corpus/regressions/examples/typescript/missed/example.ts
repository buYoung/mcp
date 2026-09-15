class Router {
  constructor(private cb: () => void) {} // S
  get callback(): () => void { return this.cb; }
  fire() {
    const fn = this.callback;
    fn(); // I
  }
}
