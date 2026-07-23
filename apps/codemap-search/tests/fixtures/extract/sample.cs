using WorkerAlias = Demo.Worker;
using static Demo.Helpers;

namespace Demo;

public interface IWorker
{
    void Execute();
}

[Obsolete]
public record Worker(string Name) : IWorker
{
    public event EventHandler? Completed;
    public string Status { get; private set; } = "ready";
    private int attempts;

    [Fact]
    public void Execute()
    {
        Helpers.Trace();
        var nested = new WorkerAlias("child");
        Completed?.Invoke(this, EventArgs.Empty);
    }
}

