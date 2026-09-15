class Router {
    Closure primary
    Closure secondary
    void setPrimary(Closure cb) { this.primary = cb; } // store_primary
    void setSecondary(Closure cb) { this.secondary = cb; } // store_secondary
    void firePrimary() { primary.call(); } // call_primary
    void fireSecondary() { secondary.call(); } // call_secondary
}
class Other {
    Closure primary
    void firePrimary() { primary.call(); } // call_other
}
