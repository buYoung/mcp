#include "meter.h"

class ProbeMeter : public Meter {
public:
    int sample(int channel) { return channel + 1; }
};
