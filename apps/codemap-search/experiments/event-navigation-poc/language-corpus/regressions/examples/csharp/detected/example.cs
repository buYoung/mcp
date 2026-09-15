using System;
class Router {
    Action cb;
    void Install(Action f) { cb = f; } // S
    void Fire() { cb(); } // I
}
