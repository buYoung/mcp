namespace Demo.Scoped;

public class Reader
{
    public int Read(Source source)
    {
        if (source.IsOpen)
        {
            return source.Next();
        }
#if HAVE_RETRY
        else
        {
            return source.Retry();
        }
#endif
        return 0;
    }

    public int Size()
    {
        return 2;
    }
}
