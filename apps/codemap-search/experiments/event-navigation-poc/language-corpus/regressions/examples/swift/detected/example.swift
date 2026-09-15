final class Router {
    var cb: () -> Void = {}
    func install(_ f: @escaping () -> Void) { cb = f } // S
    func fire() { cb() } // I
}
