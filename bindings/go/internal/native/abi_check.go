//go:build termprobe_abi_check && cgo

package native

/*
#cgo CFLAGS: -I${SRCDIR}/../../../../crates/termprobe-ffi/include
#include "termprobe.h"
*/
import "C"

import "reflect"

const nativeCompilerABIVersion = C.ABI_VERSION

type nativeLayoutPair struct{ binding, compiler reflect.Type }

// This opt-in check uses the shared header from a source checkout.
func nativeLayoutPairs() []nativeLayoutPair {
	return []nativeLayoutPair{
		{reflect.TypeFor[abiString](), reflect.TypeFor[C.TuiString]()},
		{reflect.TypeFor[abiOptionalI32](), reflect.TypeFor[C.TuiOptionalI32]()},
		{reflect.TypeFor[abiOptionalU64](), reflect.TypeFor[C.TuiOptionalU64]()},
