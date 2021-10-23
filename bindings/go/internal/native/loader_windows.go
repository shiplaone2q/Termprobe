package native

import (
	"fmt"
	"syscall"
)

type nativeLibraryHandle struct{ library *syscall.DLL }

func openNativeLibrary(path string) (*nativeLibraryHandle, error) {
	library, err := syscall.LoadDLL(path)
