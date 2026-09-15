using System;
class Router {
    public event Action Primary = delegate { };
    public event Action Secondary = delegate { };
    public void Install(Action f, Action g) {
        Primary += f; // @S1
        Secondary += g; // @S2
    }
    public void Remove(Action f) { Primary -= f; }
    public void Fire() { Primary(); } // @I1
}
