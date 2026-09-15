class Router {
  void Function() cb = () {};
  void install(void Function() f) { cb = f; } // S
  void fire() { cb.call(); } // I
}
