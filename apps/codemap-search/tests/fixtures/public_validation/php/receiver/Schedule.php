<?php

namespace Receiver;

final class Schedule
{
    // The only indexed `createFromFormat`.
    public static function createFromFormat(string $format, string $text): string
    {
        return $format . $text;
    }
}

// The path names a class that this repository does not declare.
function parse_day(string $text): \DateTimeImmutable|false
{
    return \DateTimeImmutable::createFromFormat('Y-m-d', $text);
}

// The class is held by a variable without a declared type, so this call stays unconfirmed.
function parse_with($class, string $text): mixed
{
    return $class::createFromFormat('Y-m-d', $text);
}
