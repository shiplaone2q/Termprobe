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

func (memory *nativeMemory) text(value string) abiString {
	buffer := make([]byte, len(value)+1)
	copy(buffer, value)
	pointer := &buffer[0]
	memory.pinner.Pin(pointer)
	return abiString{data: pointer, len: uintptr(len(value))}
}

func (memory *nativeMemory) optionalText(value *string) abiString {
	if value == nil {
		return abiString{}
	}
	return memory.text(*value)
}

func (memory *nativeMemory) nonemptyText(value string) abiString {
	if value == "" {
		return abiString{}
