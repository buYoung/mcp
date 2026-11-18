#include "kit/latch.h"

namespace kit {

class SpringLatch : public Latch {
public:
    bool release(int force) override { return force > 3; }
};

}  // namespace kit
