class Router {
    private Closure stored;
    private Closure secondary;
    void install(Closure f, Closure g) {
        stored = f; // @S1
        secondary = g; // @S2
    }
    Closure getCallback() { return stored; }
    Closure getOther() { return secondary; }
    void fire() { callback.call(); } // @I1
    void fireOther() { other.call(); } // @I2
}
