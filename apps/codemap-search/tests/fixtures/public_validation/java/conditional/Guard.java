package conditional;

public class Guard {
    // The call sits in the body of an if statement.
    int check(Gate gate, boolean armed) {
        if (armed) {
            return gate.latch();
        }
        return 0;
    }
}
