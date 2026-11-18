namespace Spooled;

// The base is named through a global using of another file, which is not resolved to a declaration.
public class SpooledNotifier : INotifier
{
    public bool Send(int count) => count > 1;
}
