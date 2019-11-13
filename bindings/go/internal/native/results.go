package native

import (
	"fmt"
	"math"
	"strconv"
	"time"
	"unsafe"
)

func nativeText(value abiString) string {
	if value.data == nil {
		return ""
	}
	return string(unsafe.Slice(value.data, value.len)) //nolint:gosec // G103: Copies Rust-owned bytes into a Go string before the owning result is freed.
}

func nativeOptionalText(value abiString) *string {
	if value.data == nil {
		return nil
	}
	return pointerTo(nativeText(value))
}

func readOpenResult(result *abiResult) (OpenResult, error) {
	if err := nativeError(result); err != nil {
		return OpenResult{}, err
	}
	opened := result.open
	converted := OpenResult{Session: nativeText(opened.session), Ready: opened.ready, Recording: nativeText(opened.recording)}
	if opened.shellPID.present {
		if opened.shellPID.value > math.MaxUint32 {
			return OpenResult{}, &Error{Kind: InternalError, Message: "native process identifier exceeds uint32"}
		}
		converted.ShellPID = pointerTo(uint32(opened.shellPID.value))
	}
	return converted, nil
}

func nativeInt(value abiOptionalI32) *int32 {
