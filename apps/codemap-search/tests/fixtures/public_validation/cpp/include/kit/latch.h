#pragma once

namespace kit {

class Latch {
public:
    virtual ~Latch() = default;
    virtual bool release(int force) = 0;
};

}  // namespace kit
