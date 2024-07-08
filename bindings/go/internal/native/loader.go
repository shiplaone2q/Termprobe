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
	table, err := loadNativeFunctions(path)
	if err != nil {
		return fmt.Errorf("load native engine from %q: %w", path, err)
	}
	nativeFunctions = table
	return nil
}

func loadNativeFunctions(path string) (nativeFunctionTable, error) {
	table := nativeFunctionTable{}
	library, err := openNativeLibrary(path)
	if err != nil {
		return table, err
	}
	for _, registration := range nativeRegistrations(&table) {
		if registerErr := registerNativeFunction(library, registration); registerErr != nil {
			return nativeFunctionTable{}, errors.Join(registerErr, library.close())
