class Router {
  install(f) { this.cb = f; } // S
  fire() {
    const fn = Reflect.get(this, "cb");
    fn(); // I
  }
}
