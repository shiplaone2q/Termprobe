package native

import (
	"runtime"
)

// nativeMemory pins input buffers until the synchronous Rust call returns.
// Rust copies these buffers and does not retain Go pointers.
type nativeMemory struct{ pinner runtime.Pinner }

func (memory *nativeMemory) release() {
	memory.pinner.Unpin()
}

