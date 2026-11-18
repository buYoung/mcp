<?php

class Gauge
{
    public function level(): int
    {
        return 1;
    }
}

// The receivers are declared with a nullable type.
function safe(?Gauge $gauge): int
{
    return $gauge ? $gauge->level() : 0;
}

function union(Gauge|null $gauge): int
{
    return $gauge?->level() ?? 0;
}
