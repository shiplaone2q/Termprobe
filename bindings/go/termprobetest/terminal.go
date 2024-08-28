// Package termprobetest integrates terminal sessions with Go test cleanup.
package termprobetest

import (
	"strings"
	"testing"

	termprobe "github.com/microsoft/termprobe/bindings/go"
)

// Options are per-test defaults; there is no process-wide mutable configuration.
type Options struct {
	Client  termprobe.ClientOptions
	Spawn   termprobe.SpawnOptions
	Shell   termprobe.Shell
	Program string
	Args    []string
	Prefix  string
}

// New opens a unique terminal and registers cleanup even if spawning fails.
// Test failures are reported through Close so the engine can retain configured traces.
func New(tb testing.TB, options Options) *termprobe.Client {
	tb.Helper()
	prefix := options.Prefix
	if prefix == "" {
		prefix = tb.Name()
	}
	terminal, err := termprobe.Ephemeral(prefix, options.Client)
	if err != nil {
		tb.Fatalf("create terminal: %v", err)
		return nil
	}
	tb.Cleanup(func() {
		failed := tb.Failed()
		if closeErr := terminal.Close(termprobe.CloseOptions{Failed: termprobe.Ptr(failed)}); closeErr != nil {
			tb.Errorf("close terminal: %v", closeErr)
		}
	})
	if options.Program == "" {
		_, err = terminal.Open(termprobe.OpenOptions{SpawnOptions: options.Spawn, Shell: options.Shell})
	} else {
		_, err = terminal.Run(options.Program, options.Args, options.Spawn)
	}
