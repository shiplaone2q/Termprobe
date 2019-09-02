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
