class Router {
    Runnable cb;
    void install(Runnable f) { cb = f; } // S
    void fire() { cb.run(); } // I
}
