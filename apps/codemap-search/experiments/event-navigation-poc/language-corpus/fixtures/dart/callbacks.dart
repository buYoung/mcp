class Router {
  void Function() primary = () {};
  void Function() secondary = () {};
  void setPrimary(void Function() cb) { this.primary = cb; } // store_primary
  void setSecondary(void Function() cb) { this.secondary = cb; } // store_secondary
  void firePrimary() { primary.call(); } // call_primary
  void fireSecondary() { secondary.call(); } // call_secondary
}
class Other {
  void Function() primary = () {};
  void firePrimary() { primary.call(); } // call_other
}
