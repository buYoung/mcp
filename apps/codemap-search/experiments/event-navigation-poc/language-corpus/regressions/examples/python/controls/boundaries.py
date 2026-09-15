class Router:
    def install(self, f, g):
        self.cb = f  # @S1
        self.other = g  # @S2
    def fire(self):
        fn = getattr(self, "cb")
        fn()  # @I1
    def custom(self):
        getattr = lambda obj, name: lambda: None
        fn = getattr(self, "cb")
        fn()  # @I2
