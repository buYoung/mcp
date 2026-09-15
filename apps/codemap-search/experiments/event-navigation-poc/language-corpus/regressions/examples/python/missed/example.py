class Router:
    def install(self, f):
        self.cb = f  # S
    def fire(self):
        fn = getattr(self, "cb")
        fn()  # I
