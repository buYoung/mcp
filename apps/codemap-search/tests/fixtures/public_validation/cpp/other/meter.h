#pragma once

// Never included by alias/probe_meter.cpp, which finds meter.h next to itself.
class Meter {
public:
    virtual ~Meter() = default;
    virtual int sample(int channel) = 0;
};
