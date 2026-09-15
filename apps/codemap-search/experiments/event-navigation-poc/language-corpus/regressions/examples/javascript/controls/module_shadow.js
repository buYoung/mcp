const Reflect = { get() { return () => {}; } };
class Router {
  install(f) { this.cb = f; }
  fire() {
    const fn = Reflect.get(this, "cb");
    fn();
  }
}
