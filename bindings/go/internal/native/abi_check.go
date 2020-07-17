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

