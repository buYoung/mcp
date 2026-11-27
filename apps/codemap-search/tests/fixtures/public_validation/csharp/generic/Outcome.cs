namespace Generic
{
    // The number of type arguments tells the two types apart.
    public class Outcome
    {
        public int Code() { return 0; }
    }

    public class Outcome<T>
    {
        public int Code() { return 1; }

        public int Own() { return this.Code(); }
    }
}
