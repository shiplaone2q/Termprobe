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
