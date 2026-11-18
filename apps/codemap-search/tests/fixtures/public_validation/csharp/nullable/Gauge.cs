#nullable enable
namespace Nullable
{
    public class Gauge
    {
        public int Level() { return 1; }
    }

    public class Panel
    {
        // The receivers are declared with a nullable type.
        public int Safe(Gauge? gauge) { return gauge?.Level() ?? 0; }

        public int Asserted(Gauge? gauge) { return gauge!.Level(); }
    }
}
