package native

import (
	"errors"
	"fmt"
	"sync"

	"github.com/ebitengine/purego"
)

type nativeRegistration struct {
	symbol string
	target any
}

// Registered functions and the loaded library stay live for the process lifetime.
var nativeFunctions nativeFunctionTable
var loadNativeEngine = sync.OnceValue(initializeNativeEngine)

func initializeNativeEngine() error {
	path, err := nativeLibraryPath(bundledLibraries)
	if err != nil {
		return err
	}
