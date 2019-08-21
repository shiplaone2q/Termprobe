package termprobe_test

import (
	"bufio"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"testing"
	"time"

	termprobe "github.com/microsoft/termprobe/bindings/go"
)

func TestRetryTerminalProcess(t *testing.T) {
	if !slices.Contains(os.Args, "--tui-go-retry-child") {
		return
	}
	const marker = "first-launch"
	_, err := os.Stat(marker)
	if errors.Is(err, os.ErrNotExist) {
		requireNoError(t, os.WriteFile(marker, []byte("first launch"), 0600))
	} else {
		requireNoError(t, err)
		fmt.Print("\x1b]133;B\x07retry-ready\r\n")
	}
