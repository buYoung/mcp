class Router {
  primary!: () => void; secondary!: () => void;
  setPrimary(cb: () => void) { this.primary = cb; } // store_primary
  setSecondary(cb: () => void) { this.secondary = cb; } // store_secondary
  firePrimary() { this.primary(); } // call_primary
  fireSecondary() { this.secondary(); } // call_secondary
}
class Other {
  primary!: () => void;
  firePrimary() { this.primary(); } // call_other
}
