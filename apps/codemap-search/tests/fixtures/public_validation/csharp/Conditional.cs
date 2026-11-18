namespace Conditional
{
    public class Gate
    {
        public int Latch() { return 1; }
    }

    public class Guard
    {
        // The call sits in the body of an if statement.
        public int Check(Gate gate, bool armed)
        {
            if (armed)
            {
                return gate.Latch();
            }
            return 0;
        }
    }
}
