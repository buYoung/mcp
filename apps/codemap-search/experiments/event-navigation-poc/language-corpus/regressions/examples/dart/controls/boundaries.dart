class Router {
  (void Function(), void Function()) callbacks = (() {}, () {});
  void install(void Function() f, void Function() g) {
    callbacks = (f, g); // @S1
  }
  void first() {
    final (callback, _) = callbacks;
    callback(); // @I1
  }
  void second() {
    final (_, callback) = callbacks;
    callback(); // @I2
  }
}
