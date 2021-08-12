package native

import (
	"fmt"
	"syscall"
)

type nativeLibraryHandle struct{ library *syscall.DLL }

