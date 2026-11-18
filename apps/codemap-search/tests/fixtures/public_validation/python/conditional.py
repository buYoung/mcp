class Gate:
    def latch(self) -> int:
        return 1

    # The call sits in the body of an if statement.
    def check(self, armed: bool) -> int:
        if armed:
            return self.latch()
        return 0
