package signature

import context "example.com/other/context"

type OtherStore struct{}

func (s *OtherStore) Lookup(ctx context.Context, key string) (Record, error) {
	return Record{Key: key}, nil
}

func (s *OtherStore) Persist(ctx context.Context, record Record) error {
	return nil
}
