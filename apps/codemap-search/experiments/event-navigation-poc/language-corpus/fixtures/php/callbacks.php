<?php
class Router {
    private $primary;
    private $secondary;
    function setPrimary($cb) { $this->primary = $cb; } // store_primary
    function setSecondary($cb) { $this->secondary = $cb; } // store_secondary
    function firePrimary() { ($this->primary)(); } // call_primary
    function fireSecondary() { ($this->secondary)(); } // call_secondary
}
class Other {
    private $primary;
    function firePrimary() { ($this->primary)(); } // call_other
}
