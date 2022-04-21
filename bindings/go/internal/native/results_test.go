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
		cursor:     abiCursor{x: 3, y: 4, visible: true, shape: memory.text("bar"), color: memory.text("#abcdef")},
		exitSignal: memory.text("KILL"),
		modes:      &modes[0], modesLen: 1,
		mouseMode: memory.text("drag"),
		colors: abiTerminalColors{
			foreground: memory.text("#ffffff"), background: memory.text("#000000"), cursor: memory.text("#abcdef"),
			palette: &palette[0], paletteLen: 1,
		},
	})
	if state.Cursor.Shape != "bar" || state.Cursor.Color != "#abcdef" || !state.Cursor.Visible {
		t.Fatalf("cursor=%+v", state.Cursor)
	}
	if state.ExitSignal == nil || *state.ExitSignal != "KILL" || state.MouseMode != "drag" || state.Modes["cursor_visible"] {
		t.Fatalf("state=%+v", state)
	}
	if state.Colors.Palette[1] != "#123456" || state.Colors.Foreground != "#ffffff" {
		t.Fatalf("colors=%+v", state.Colors)
