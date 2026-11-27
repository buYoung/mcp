#include <algorithm>

namespace receiver {

struct Ledger {
    // The only indexed `max`.
    static int max(int left, int right) { return left > right ? left : right; }
};

// The path names a namespace that this repository does not declare.
int clamp_total(int total) { return std::max(total, 0); }

template <typename Limits>
int limit_total(int total) { return Limits::max(total, 0); }

}  // namespace receiver
