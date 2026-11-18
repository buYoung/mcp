#pragma once

class Counter {
public:
    int sample(int channel) { return channel; }
};

using Meter = Counter;
