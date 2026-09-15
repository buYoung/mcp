class Router {
    Runnable primary, secondary;
    void setPrimary(Runnable cb) { this.primary = cb; } // store_primary
    void setSecondary(Runnable cb) { this.secondary = cb; } // store_secondary
    void firePrimary() { primary.run(); } // call_primary
    void fireSecondary() { secondary.run(); } // call_secondary
}
class Other {
    Runnable primary;
    void firePrimary() { primary.run(); } // call_other
}
