package native

import "github.com/ebitengine/purego"

func nativeLibc() string {
	// Inspect the running process, not the build host or the distribution name.
