using System;

namespace Shop
{
    public interface IStore
    {
        void Put(string key, object value);

        Item Find(Guid id);

        void Track(Type kind);

        void Audit(Entry entry);
    }

    public class Item { }

    public class Entry { }
}
