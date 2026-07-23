using Job = Demo.Job;
public class Worker
{
    public void Run(Queue queue, Processor processor)
    {
        var job = queue.Next();
        processor.Process(job);
        queue.Ack(job);
    }
}

