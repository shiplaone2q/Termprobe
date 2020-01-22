package native

import "testing"

func TestNativeStatePreservesRichTerminalMetadata(t *testing.T) {
	memory := nativeMemory{}
	defer memory.release()
	modes := []abiMode{{name: memory.text("cursor_visible"), enabled: false}}
	palette := []abiPair{{key: memory.text("1"), value: memory.text("#123456")}}
	memory.pinner.Pin(&modes[0])
	memory.pinner.Pin(&palette[0])
	state := nativeState(abiState{
