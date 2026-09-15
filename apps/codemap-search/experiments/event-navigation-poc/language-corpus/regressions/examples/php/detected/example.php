<?php
class Router {
    private $cb;
    function install($f) { $this->cb = $f; } // S
    function fire() { ($this->cb)(); } // I
}
