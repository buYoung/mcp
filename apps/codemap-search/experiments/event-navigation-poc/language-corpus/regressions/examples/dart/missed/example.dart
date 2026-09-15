class Router {
  (void Function(), void Function()) callbacks = (() {}, () {});
  void install(void Function() f) {
    callbacks = (f, () {}); // S
  }
  void fire() {
    final (first, _) = callbacks;
    first(); // I
  }
}
