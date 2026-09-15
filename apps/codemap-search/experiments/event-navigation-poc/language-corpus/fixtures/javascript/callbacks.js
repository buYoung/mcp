class Router {
  primary; secondary;
  setPrimary(cb) { this.primary = cb; } // store_primary
  setSecondary(cb) { this.secondary = cb; } // store_secondary
  firePrimary() { this.primary(); } // call_primary
  fireSecondary() { this.secondary(); } // call_secondary
}
class Other {
  primary;
  firePrimary() { this.primary(); } // call_other
}
