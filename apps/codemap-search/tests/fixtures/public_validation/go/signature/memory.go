package signature

import "context"

type MemoryStore struct{}

func (s *MemoryStore) Lookup(ctx context.Context, key string) (Record, error) {
	return Record{Key: key}, nil
}

func (s *MemoryStore) Persist(ctx context.Context, record Record) error {
	return nil
}
