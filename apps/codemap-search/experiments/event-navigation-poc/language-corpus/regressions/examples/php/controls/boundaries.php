<?php
class Router {
    private $cb;
    private $other;
    private $name;
    function install($f, $g) {
        $this->cb = $f; // @S1
        $this->other = $g; // @S2
        $this->name = $f; // @S3
    }
    function fire() {
        $name = 'cb';
        ($this->$name)(); // @I1
    }
    function unresolved($name) {
        ($this->$name)(); // @I2
    }
}
