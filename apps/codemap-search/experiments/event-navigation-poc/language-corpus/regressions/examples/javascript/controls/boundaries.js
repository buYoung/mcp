class Router {
  install(f, g) {
    this.cb = f; // @S1
    this.other = g; // @S2
  }
  fire() {
    const fn = Reflect.get(this, "cb");
    fn(); // @I1
  }
  custom() {
    const Reflect = { get() { return () => {}; } };
    const fn = Reflect.get(this, "cb");
    fn(); // @I2
  }
}
