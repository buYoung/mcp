<?php
class Router {
    private array $items = [];
    private array $cache = [];
    function install($callback) { $this->items[] = $callback; } // @S1
    function prepare() {
        foreach ($this->items as $callback) {
            $slot = &$this->cache[];
            $slot = $callback(...); // @I2
        }
    }
    function fire() {
        foreach ($this->cache as $callback) {
            $callback(); // @I1
        }
    }
}
