namespace Receiver;

public static class TextExtensions
{
    // The only indexed `Shouted`: it extends a type that this repository does not declare.
    public static string Shouted(this string text) => text.ToUpperInvariant();
}

public static class Banner
{
    // Member syntax on a value of the extended type reaches the extension method.
    public static string Title(string text) => text.Shouted();
}
