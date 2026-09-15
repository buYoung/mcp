class Router {
    var primary: () -> Void = {}
    var secondary: () -> Void = {}
    func setPrimary(_ cb: @escaping () -> Void) { self.primary = cb } // store_primary
    func setSecondary(_ cb: @escaping () -> Void) { self.secondary = cb } // store_secondary
    func firePrimary() { primary() } // call_primary
    func fireSecondary() { secondary() } // call_secondary
}
class Other {
    var primary: () -> Void = {}
    func firePrimary() { primary() } // call_other
}
