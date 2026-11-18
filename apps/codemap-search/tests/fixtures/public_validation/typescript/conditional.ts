export class Gate {
  latch(): number {
    return 1;
  }
}

// The call sits in the body of an if statement.
export function check(gate: Gate, armed: boolean): number {
  if (armed) {
    return gate.latch();
  }
  return 0;
}
