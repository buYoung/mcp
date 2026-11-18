using System.Collections.Generic;

namespace Partial;

public class EmailNotifier : INotifier
{
    public bool Send(int count) => count > 0;
}

// A generic type implements the declaration like any other type.
public class BufferedNotifier<T> : INotifier
{
    private readonly List<T> pending = new();

    public bool Send(int count) => pending.Count >= count;
}
