//go:build termprobe_abi_check && cgo

package native

import (
	"go/ast"
	"go/parser"
	"go/token"
	"reflect"
	"strings"
	"testing"
)

func TestNativeLayoutMatchesCompiler(t *testing.T) {
	if abiVersion != nativeCompilerABIVersion {
		t.Errorf("ABI version: Go=%d, C=%d", abiVersion, nativeCompilerABIVersion)
	}
	checked := make(map[string]bool)
	for _, pair := range nativeLayoutPairs() {
		checked[pair.binding.Name()] = true
		t.Run(pair.binding.Name(), func(t *testing.T) {
			if pair.binding.Size() != pair.compiler.Size() {
				t.Errorf("size: Go=%d, C=%d", pair.binding.Size(), pair.compiler.Size())
			}
			if pair.binding.Align() != pair.compiler.Align() {
				t.Errorf("alignment: Go=%d, C=%d", pair.binding.Align(), pair.compiler.Align())
			}
			fields := nativeFields(pair.compiler)
			for name, field := range nativeFields(pair.binding) {
				native, found := fields[name]
				if !found {
					t.Errorf("field %s has no C counterpart", field.Name)
					continue
				}
				if field.Offset != native.Offset || field.Type.Size() != native.Type.Size() {
					t.Errorf("field %s: Go offset/size=%d/%d, C=%d/%d",
						field.Name, field.Offset, field.Type.Size(), native.Offset, native.Type.Size())
				}
				delete(fields, name)
			}
