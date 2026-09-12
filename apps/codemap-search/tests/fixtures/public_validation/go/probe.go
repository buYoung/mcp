package validation

import (
	"os"
	"sync/atomic"
)

type Unrelated struct{}

func (Unrelated) len() int { return 0 }
func (Unrelated) Expand() {}
func Load()              {}

func StandardBuiltin(values []int) int {
	return len(values)
}

func StandardPackage(value string) string {
	return os.Expand(value, func(key string) string { return key })
}

func AtomicMethod(value *atomic.Int64) int64 {
	return value.Load()
}

const (
	ExplicitValue = 1
	ImplicitValue
)

func ImplicitConstant() int {
	return ImplicitValue
}
