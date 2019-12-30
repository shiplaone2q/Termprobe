package native

import (
	"strings"
	"testing"
)

func TestNativeLibraryPathRequiresBundleOrOverride(t *testing.T) {
	t.Setenv("TERMPROBE__NATIVE_LIBRARY", "")
