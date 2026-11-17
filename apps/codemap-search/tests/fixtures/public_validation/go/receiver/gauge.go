package receiver

import (
	"strings"
	"sync/atomic"
)

// Gauge declares the only indexed CompareAndSwap.
type Gauge struct{ level int64 }

func (gauge *Gauge) CompareAndSwap(previous, next int64) bool {
	if gauge.level != previous {
		return false
	}
	gauge.level = next
	return true
}

// Grow is the only indexed Grow, a package-level function.
func Grow(size int) int { return size * 2 }

// The receivers below are declared with types of other packages.
func SwapCounter(counter *atomic.Int64) bool {
	return counter.CompareAndSwap(1, 2)
}

func GrowBuilder(builder *strings.Builder) {
	builder.Grow(8)
}

// The receiver type is not written here, so this call stays unconfirmed.
func SwapCurrent() bool {
	current := Current()
	return current.CompareAndSwap(1, 2)
}

func Current() *Gauge { return &Gauge{} }
