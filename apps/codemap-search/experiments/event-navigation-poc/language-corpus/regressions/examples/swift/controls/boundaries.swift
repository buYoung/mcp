final class Router {
    var callbacks: (() -> Void, () -> Void) = ({}, {})
    func install(_ f: @escaping () -> Void, _ g: @escaping () -> Void) {
        (callbacks.0, callbacks.1) = (f, g) // @S1
    }
    func first() { callbacks.0() } // @I1
    func second() { callbacks.1() } // @I2
}
