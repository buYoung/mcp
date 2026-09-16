#include <vector>
using Callback = void (*)();
struct Store {
    using Entries = std::vector<Callback>;
    Entries entries;
    void fire() { entries[0](); } // @I1
};
struct Sink {
    Store *store;
    Sink(Store &source) : store{&source} {}
    void install(Callback callback) { store->entries.push_back(callback); } // @S1
};
struct Pair {
    Store left, right;
    void install(Callback callback) { left.entries.push_back(callback); } // @S2
    void fireRight() { right.entries[0](); } // @I2
    void fireLeft() { left.entries[0](); } // @I3
};
struct Rebind {
    Callback primary, secondary;
    void alias() { primary = secondary; }
    void install(Callback callback) { primary = callback; } // @S3
    void fireOther() { secondary(); } // @I4
};
