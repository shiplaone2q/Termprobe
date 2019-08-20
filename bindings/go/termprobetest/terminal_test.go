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
