#include <functional>
struct Router {
    std::function<void()> cb;
    void install(std::function<void()> f) { cb = f; } // S
    void fire() { cb(); } // I
};
