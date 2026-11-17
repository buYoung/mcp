package signature

import "context"

type Store interface {
	Lookup(ctx context.Context, key string) (Record, error)
	Persist(ctx context.Context, record Record) error
}

type Record struct {
	Key string
}
