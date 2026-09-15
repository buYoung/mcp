class Router {
  install(f) { this.cb = f; } // S
  fire() { this.cb(); } // I
}
