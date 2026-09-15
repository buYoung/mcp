class Router:
    def set_primary(self, cb):
        self.primary = cb  # store_primary
    def set_secondary(self, cb):
        self.secondary = cb  # store_secondary
    def fire_primary(self):
        self.primary()  # call_primary
    def fire_secondary(self):
        self.secondary()  # call_secondary
class Other:
    def fire_primary(self):
        self.primary()  # call_other
