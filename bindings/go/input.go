package termprobe

// Keyboard sends terminal key events, including separate press and release events.
type Keyboard struct{ runtime *nativeRuntime }

func (keyboard *Keyboard) Press(keys ...string) error  { return keyboard.runtime.key(keys, 0) }
func (keyboard *Keyboard) Down(keys ...string) error   { return keyboard.runtime.key(keys, 1) }
