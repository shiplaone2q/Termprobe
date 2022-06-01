package native

import (
	"fmt"
	"syscall"
)

type nativeLibraryHandle struct{ library *syscall.DLL }

func openNativeLibrary(path string) (*nativeLibraryHandle, error) {
	library, err := syscall.LoadDLL(path)
	if err != nil {
		return nil, fmt.Errorf("load native library %q: %w", path, err)
	}
	return &nativeLibraryHandle{library: library}, nil
}
