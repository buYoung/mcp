class Router {
  static callback: () => void;
  callback: () => void;
  static install(value: () => void) { this.callback = value; } // @S1
  static fire() { this.callback(); } // @I1
  fire() { this.callback(); } // @I2
}
