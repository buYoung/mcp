class Router {
    private Closure stored
    void install(Closure f) { stored = f; } // S
    Closure getCallback() { return stored; }
    void fire() { callback.call(); } // I
}
