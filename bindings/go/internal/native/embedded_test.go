package native

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
)

func libraryFixture(t *testing.T, contents string) bundledLibrary {
	t.Helper()
	var buffer bytes.Buffer
	compressed := gzip.NewWriter(&buffer)
	archive := tar.NewWriter(compressed)
	name := "library.so"
	if err := archive.WriteHeader(&tar.Header{Name: name, Mode: 0o600, Size: int64(len(contents))}); err != nil {
		t.Fatal(err)
	}
	if _, err := archive.Write([]byte(contents)); err != nil {
		t.Fatal(err)
	}
	if err := archive.Close(); err != nil {
		t.Fatal(err)
	}
	if err := compressed.Close(); err != nil {
		t.Fatal(err)
	}
	return bundledLibrary{name: name, checksum: fmt.Sprintf("%x", sha256.Sum256([]byte(contents))), archive: buffer.Bytes()}
}

func TestBundleTarget(t *testing.T) {
	for _, tt := range []struct{ os, arch, libc, target string }{
		{"linux", "amd64", "gnu", "x86_64-unknown-linux-gnu"},
		{"linux", "arm64", "gnu", "aarch64-unknown-linux-gnu"},
		{"linux", "amd64", "musl", "x86_64-unknown-linux-musl"},
		{"linux", "arm64", "musl", "aarch64-unknown-linux-musl"},
		{"darwin", "amd64", "", "x86_64-apple-darwin"},
		{"darwin", "arm64", "", "aarch64-apple-darwin"},
		{"windows", "amd64", "", "x86_64-pc-windows-msvc"},
		{"windows", "arm64", "", ""},
		{"linux", "386", "gnu", ""},
		{"linux", "amd64", "other", ""},
		{"freebsd", "amd64", "", ""},
	} {
		t.Run(tt.os+"/"+tt.arch+"/"+tt.libc, func(t *testing.T) {
			target, err := bundleTarget(tt.os, tt.arch, tt.libc)
			if target != tt.target || (err != nil) != (tt.target == "") {
				t.Fatalf("target = %q, error = %v", target, err)
			}
		})
	}
}

func TestOverrideBypassesBundleAndCache(t *testing.T) {
	t.Setenv("TERMPROBE__NATIVE_LIBRARY", "/explicit/library")
	path, err := nativeLibraryPath(nil)
	if err != nil || path != "/explicit/library" {
		t.Fatalf("path = %q, error = %v", path, err)
	}
}

func TestNativeLibraryPathSelectsProcessTarget(t *testing.T) {
	t.Setenv("TERMPROBE__NATIVE_LIBRARY", "")
	cache := t.TempDir()
	t.Setenv("HOME", cache)
