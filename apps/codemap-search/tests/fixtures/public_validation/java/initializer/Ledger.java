package initializer;

public class Ledger {
    int balance() { return 1; }

    // The call gives a local variable its first value.
    int total() {
        int base = balance();
        return base;
    }
}
