using System;
using System.Threading;
class Box {
    private Action callback;
    public Box(Action value) { callback = value; }
    public Action Value => callback;
}
class Router {
    private Box[] slots = new Box[0];
    private Box[] other = new Box[0];
    public void Install(Action callback) {
        var next = new Box[1];
        next[0] = new Box(callback); // @S1
        Interlocked.CompareExchange(ref slots, next, slots);
    }
    public void Fire() {
        var view = Volatile.Read(ref slots);
        view[0].Value(); // @I1
    }
    public void FireOther() {
        var view = Volatile.Read(ref other);
        view[0].Value(); // @I2
    }
}
