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
	scanner := bufio.NewScanner(os.Stdin)
	for scanner.Scan() {
		fmt.Printf("received:%s\r\n", scanner.Text())
	}
	requireNoError(t, scanner.Err())
	os.Exit(0)
}

func clientLaunchTimeouts() termprobe.Timeouts {
	return termprobe.Timeouts{
		Text: termprobe.Ptr(11 * time.Millisecond), Idle: termprobe.Ptr(12 * time.Millisecond),
		Command: termprobe.Ptr(13 * time.Millisecond), Exit: termprobe.Ptr(14 * time.Millisecond),
