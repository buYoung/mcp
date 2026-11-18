namespace TypedLocal
{
    public class Meter
    {
        public int Read() { return 1; }
    }

    public class Station
    {
        public Meter Make() { return new Meter(); }

        // The receiver is a local variable with a written type.
        public int FromLocal()
        {
            Meter meter = Make();
            return meter.Read();
        }

        // The type of this local variable is not written.
        public int Inferred()
        {
            var meter = Make();
            return meter.Read();
        }
    }
}
