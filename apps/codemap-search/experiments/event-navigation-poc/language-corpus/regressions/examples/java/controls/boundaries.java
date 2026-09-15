interface Work { void execute(); }
class Router {
    Runnable primary, secondary;
    void install(Runnable f, Runnable g) {
        primary = f; // @S1
        secondary = g; // @S2
    }
    void fire() {
        Runnable relay = primary::run;
        relay.run(); // @I1
        relay.toString(); // @I2
    }
    void custom() {
        Work relay = secondary::run;
        relay.execute(); // @I3
    }
}
