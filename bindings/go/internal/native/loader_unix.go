//go:build linux || darwin || freebsd || netbsd

package native

import (
	"fmt"

	"github.com/ebitengine/purego"
)

type nativeLibraryHandle struct{ library uintptr }

func openNativeLibrary(path string) (*nativeLibraryHandle, error) {
