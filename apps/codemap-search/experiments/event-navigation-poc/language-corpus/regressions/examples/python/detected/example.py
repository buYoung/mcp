class Router:
    def install(self, f):
        self.cb = f  # S
    def fire(self):
        self.cb()  # I
