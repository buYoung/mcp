#include <functional>
struct Router {
    std::function<void()> primary, secondary;
    void set_primary(std::function<void()> cb) { this->primary = cb; } // store_primary
    void set_secondary(std::function<void()> cb) { this->secondary = cb; } // store_secondary
    void fire_primary() { primary(); } // call_primary
    void fire_secondary() { secondary(); } // call_secondary
};
struct Other {
    std::function<void()> primary;
    void fire_primary() { primary(); } // call_other
};
