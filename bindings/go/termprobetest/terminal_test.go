package termprobetest_test

import (
	"bufio"
	"fmt"
	"os"
	"slices"
	"testing"
	"time"

	termprobe "github.com/microsoft/termprobe/bindings/go"
	"github.com/microsoft/termprobe/bindings/go/termprobetest"
)

func TestHelperChild(t *testing.T) {
	if !slices.Contains(os.Args, "--helper-child") {
		return
	}
	fmt.Println("helper-ready")
	scanner := bufio.NewScanner(os.Stdin)
	for scanner.Scan() {
		if scanner.Text() == "quit" {
			os.Exit(0)
		}
	}
	os.Exit(0)
}

func helperOptions(t *testing.T) termprobetest.Options {
	t.Helper()
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	return termprobetest.Options{
		Client:  termprobe.ClientOptions{Recording: &termprobe.AutomaticRecording{Mode: termprobe.RecordingDisabled}},
		Spawn:   termprobe.SpawnOptions{WaitReady: termprobe.Ptr(false)},
		Program: executable, Args: []string{"-test.run=^TestHelperChild$", "--", "--helper-child"},
	}
}

func TestCleanupAndParallelIsolation(t *testing.T) {
	names := make(chan string, 2)
	t.Cleanup(func() {
		close(names)
		verifyClosedSessions(t, names)
	})
	for _, name := range []string{"one", "two"} {
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			terminal := termprobetest.New(t, helperOptions(t))
