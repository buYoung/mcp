class Router {
    Closure cb
    void install(Closure f) { cb = f; } // S
    void fire() { cb.call(); } // I
}
