struct Bag<T> {
    var value: T?
    mutating func insert(_ item: T) { value = item }
}
final class Router {
#if DEBUG
    let debug = true
#endif
    var bag = Bag<() -> Void>()
    var other = Bag<() -> Void>()
    func install(_ callback: @escaping () -> Void) {
        bag.insert(callback) // @S1
    }
    func fire() { bag.value?() } // @I1
    func fireOther() { other.value?() } // @I2
}
