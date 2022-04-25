package termprobe_test

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"image/png"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"

	termprobe "github.com/microsoft/termprobe/bindings/go"
)

func TestTerminalProcess(t *testing.T) {
	if !slices.Contains(os.Args, "--tui-go-child") {
		return
	}
	fmt.Print("\x1b[2J\x1b[Hgo-ready\r\nitem item\r\n\x1b[1mstyled\x1b[0m plain\r\n界 café\r\n\x1b]8;;https://example.test/docs\x1b\\linked docs\x1b]8;;\x1b\\\r\n")
	scanner := bufio.NewScanner(os.Stdin)
	for scanner.Scan() {
		switch scanner.Text() {
		case "quit":
			os.Exit(0)
		case "events":
			fmt.Print("\x1b]2;Go title\x07\x1b]52;c;Z28tY2xpcGJvYXJk\x07\x07\r\nevents-done\r\n")
		case "history":
			for i := 0; i < 100; i++ {
				fmt.Printf("history-%d\r\n", i)
			}
		case "raster":
			fmt.Print("\x1b[2J\x1b[Hgo-raster-ready\r\n")
		default:
			fmt.Printf("received:%s\r\n", scanner.Text())
		}
	}
	os.Exit(0)
}

func TestNativeScrollback(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{Backend: termprobe.Alacritty})
	requireNoError(t, terminal.Submit(termprobe.Ptr("history")))
	requireNoError(t, terminal.GetByText("history-99", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{}))
	full, err := terminal.Text(termprobe.TextOptions{Full: true})
	requireNoError(t, err)
	bottom, err := terminal.GetViewport()
	requireNoError(t, err)
	if bottom.Offset != 0 || bottom.History == 0 {
		t.Fatalf("unexpected bottom viewport: %+v", bottom)
	}
	requireNoError(t, terminal.Scroll(2))
	requireNoError(t, terminal.Scroll(-1))
	view, err := terminal.GetViewport()
	requireNoError(t, err)
	if view.Offset != 1 {
		t.Fatalf("negative scroll did not move down: %+v", view)
	}
	requireNoError(t, terminal.ScrollToTop())
	view, err = terminal.GetViewport()
	requireNoError(t, err)
	if view.Offset != view.History {
		t.Fatalf("unexpected top viewport: %+v", view)
	}
	stillFull, err := terminal.Text(termprobe.TextOptions{Full: true})
	requireNoError(t, err)
	if stillFull != full {
		t.Fatal("native scroll changed full text")
	}
	requireNoError(t, terminal.ScrollToBottom())
	view, err = terminal.GetViewport()
	requireNoError(t, err)
	if view.Offset != 0 {
		t.Fatalf("bottom did not restore live screen: %+v", view)
	}
	requireNoError(t, terminal.Scroll(2147483647))
	requireNoError(t, terminal.Scroll(-2147483648))
	view, err = terminal.GetViewport()
	requireNoError(t, err)
	if view.Offset != 0 {
		t.Fatalf("signed scroll limits did not clamp: %+v", view)
	}
}

func newTerminal(t *testing.T, options termprobe.ClientOptions) *termprobe.Client {
	t.Helper()
	terminal := newClient(t, options)
	runTerminal(t, terminal, termprobe.SpawnOptions{})
	return terminal
}

func newClient(t *testing.T, options termprobe.ClientOptions) *termprobe.Client {
	t.Helper()
	terminal, err := termprobe.Ephemeral(t.Name(), options)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := terminal.Close(); err != nil {
			t.Errorf("close terminal: %v", err)
		}
	})
	return terminal
}
func runTerminal(t *testing.T, terminal *termprobe.Client, options termprobe.SpawnOptions) {
	t.Helper()
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	options.WaitReady = termprobe.Ptr(false)
	if _, err = terminal.Run(executable, []string{"-test.run=^TestTerminalProcess$", "--", "--tui-go-child"}, options); err != nil {
		t.Fatal(err)
	}
