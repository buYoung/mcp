using System;
class Router {
    public event Action Fired = delegate { };
    public void Install(Action f) { Fired += f; } // S
    public void Fire() { Fired(); } // I
}
