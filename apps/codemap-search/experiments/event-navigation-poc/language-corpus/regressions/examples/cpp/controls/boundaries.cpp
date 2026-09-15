#include <functional>
struct Router {
    std::function<void()> primary, secondary;
    void install(std::function<void()> f, std::function<void()> g) {
        primary = f; // @S1
        secondary = g; // @S2
    }
    void fire() {
        auto relay = [this] { primary(); }; // @I1
        relay();
    }
    void unused() {
        auto relay = [this] { primary(); }; // @I2
    }
    void copied() {
        auto cb = secondary;
        auto relay = [cb] { cb(); }; // @I3
        relay();
    }
};
