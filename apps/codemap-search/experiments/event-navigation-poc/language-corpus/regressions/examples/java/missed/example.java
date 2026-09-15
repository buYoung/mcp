class Router {
    Runnable cb;
    void install(Runnable f) { cb = f; } // S
    void fire() {
        Runnable relay = cb::run;
        relay.run(); // I
    }
}
