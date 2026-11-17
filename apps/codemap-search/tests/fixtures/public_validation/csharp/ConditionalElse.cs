namespace Demo
{
    public class Writer
    {
        private readonly Sink _sink;

        public void Close()
        {
            if (_sink.IsOpen)
            {
                _sink.Flush();
            }
#if HAVE_RESET
            else
            {
                _sink.Reset();
            }
#endif
            _sink.Dispose();
        }

        public int Count()
        {
            return 1;
        }
    }
}
