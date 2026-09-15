final class Router {
    var callbacks: (() -> Void, () -> Void) = ({}, {})
    func install(_ f: @escaping () -> Void) {
        (callbacks.0, callbacks.1) = (f, {}) // S
    }
    func fire() { callbacks.0() } // I
}
