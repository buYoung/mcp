<?php
use Demo\Rule;
function allow(Rule $rule): bool
{
    return $rule->check();
}

