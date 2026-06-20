/** Fixture exercising C++ header (.hpp) branch-sensitive extraction. */

#pragma once

/** A point value type (struct members default to public/exported). */
struct Point {
    int x;

    /** Return a label for the point. */
    const char *label() const {
        return "point";
    }
};

/** A service with a public and a private member. */
class Service {
public:
    /** Start the service. */
    void start();

    /**
     * @deprecated use start instead
     */
    void startLegacy();

private:
    int hiddenState();
};
