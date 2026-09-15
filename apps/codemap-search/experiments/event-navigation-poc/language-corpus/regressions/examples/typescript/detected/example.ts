class Router {
  cb!: () => void;
  install(f: () => void) { this.cb = f; } // S
  fire() { this.cb(); } // I
}
