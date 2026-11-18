namespace Generic
{
    public abstract class Codec<T>
    {
        public abstract T Decode(string raw);

        public int Width() { return 1; }
    }

    public class TextCodec : Codec<string>
    {
        public override string Decode(string raw) { return raw.Trim(); }
    }

    public class Reader
    {
        // The receiver's declared type carries a type argument.
        public int Measure(Codec<string> codec) { return codec.Width(); }

        public string Parse(Codec<string> codec) { return codec.Decode("a"); }
    }
}
