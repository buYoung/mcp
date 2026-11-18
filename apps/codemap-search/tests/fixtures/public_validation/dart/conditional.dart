class Gate {
  int latch() => 1;
}

// The call sits in the body of an if statement.
int check(Gate gate, bool armed) {
  if (armed) {
    return gate.latch();
  }
  return 0;
}
