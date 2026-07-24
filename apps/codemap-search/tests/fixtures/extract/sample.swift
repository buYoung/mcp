import Foundation

public struct Worker {
    public let name = "swift"

    @available(*, deprecated)
    public func run() {
        Helper.start()
    }
}
