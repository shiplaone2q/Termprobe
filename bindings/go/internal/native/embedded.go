package native

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"crypto/rand"
	"crypto/sha256"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
)

type bundledLibrary struct {
	name     string
	checksum string
	archive  []byte
}

// Release packaging adds go:embed files for the target OS and architecture.
// Source checkouts leave this map empty and use TERMPROBE__NATIVE_LIBRARY.
var bundledLibraries = map[string]bundledLibrary{}

func nativeLibraryPath(libraries map[string]bundledLibrary) (string, error) {
	if path := os.Getenv("TERMPROBE__NATIVE_LIBRARY"); path != "" {
		return path, nil
	}
	target, err := bundleTarget(runtime.GOOS, runtime.GOARCH, nativeLibc())
	if err != nil {
		return "", err
	}
	library, ok := libraries[target]
	if !ok {
		return "", fmt.Errorf("no bundled native engine for %s: install a released Go module or set TERMPROBE__NATIVE_LIBRARY to a library from the same source revision", target)
	}
	cache, err := os.UserCacheDir()
	if err != nil {
		return "", fmt.Errorf("locate native engine cache: %w", err)
	}
	return cacheLibrary(cache, library)
}

func bundleTarget(goos, goarch, libc string) (string, error) {
	arch := map[string]string{"amd64": "x86_64", "arm64": "aarch64"}[goarch]
	if arch != "" {
		switch goos {
		case "darwin":
			return arch + "-apple-darwin", nil
		case "linux":
			if libc == "gnu" || libc == "musl" {
				return arch + "-unknown-linux-" + libc, nil
			}
		case "windows":
			if goarch == "amd64" {
				return arch + "-pc-windows-msvc", nil
			}
		}
	}
	return "", fmt.Errorf("no bundled native engine supports %s/%s with libc %q", goos, goarch, libc)
}

func cacheLibrary(cache string, library bundledLibrary) (path string, resultErr error) {
	directory := filepath.Join(cache, "termprobe", "native", library.checksum)
	if err := os.MkdirAll(directory, 0o700); err != nil {
		return "", fmt.Errorf("create native engine cache: %w", err)
	}
	root, err := os.OpenRoot(directory)
	if err != nil {
		return "", fmt.Errorf("open native engine cache: %w", err)
	}
	defer func() { resultErr = errors.Join(resultErr, root.Close()) }()
	path = filepath.Join(directory, library.name)
	if err := verifyCachedLibrary(root, library); err == nil {
		return path, nil
	} else if !errors.Is(err, os.ErrNotExist) {
		return "", fmt.Errorf("check native engine cache %q: %w", path, err)
	}

	temporaryName := ".engine-" + rand.Text()
	file, err := root.OpenFile(temporaryName, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return "", fmt.Errorf("create temporary native engine: %w", err)
	}
	defer func() {
		if err := root.Remove(temporaryName); err != nil {
			resultErr = errors.Join(resultErr, fmt.Errorf("remove temporary native engine: %w", err))
