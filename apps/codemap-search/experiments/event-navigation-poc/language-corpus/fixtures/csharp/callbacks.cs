using System;
class Router {
    Action primary, secondary;
    void SetPrimary(Action cb) { this.primary = cb; } // store_primary
    void SetSecondary(Action cb) { this.secondary = cb; } // store_secondary
    void FirePrimary() { primary(); } // call_primary
    void FireSecondary() { secondary(); } // call_secondary
}
class Other {
    Action primary;
    void FirePrimary() { primary(); } // call_other
}
