//go:build linux || darwin || freebsd || netbsd

package native

import (
	"fmt"

	"github.com/ebitengine/purego"
)

type nativeLibraryHandle struct{ library uintptr }

func openNativeLibrary(path string) (*nativeLibraryHandle, error) {
	library, err := purego.Dlopen(path, purego.RTLD_NOW|purego.RTLD_LOCAL)
	if err != nil {
