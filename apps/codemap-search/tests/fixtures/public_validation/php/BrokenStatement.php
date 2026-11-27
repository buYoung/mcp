<?php

namespace Demo;

class Report
{
    public function build(array $rows): array
    {
        $total = count($rows);
        $broken = $total + ;
        return $rows;
    }

    public function size(): int
    {
        return 1;
    }
}
