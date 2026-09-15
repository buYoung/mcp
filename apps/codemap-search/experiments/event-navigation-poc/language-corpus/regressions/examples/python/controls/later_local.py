class Router:
    def install(self, f):
        self.cb = f
    def fire(self):
        fn = getattr(self, "cb")
        fn()
        getattr = lambda obj, name: lambda: None
