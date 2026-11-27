using System;
using Shop;

namespace Shop.Stores
{
    public class Entry { }

    public abstract class MemoryStore : IStore
    {
        public void Put(string key, object value) { }

        public Item Find(Guid id) { return null; }

        public void Track(Type kind) { }

        public void Audit(Entry entry) { }

        public abstract void Audit(Shop.Entry entry);
    }
}
