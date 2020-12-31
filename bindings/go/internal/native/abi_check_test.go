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
