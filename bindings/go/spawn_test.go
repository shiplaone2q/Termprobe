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
		Ready: termprobe.Ptr(15 * time.Millisecond),
	}
}

type launchTimeoutCase struct {
	name     string
	options  termprobe.Timeouts
	expected termprobe.EffectiveTimeouts
}

func launchTimeoutCases() []launchTimeoutCase {
	zero := time.Duration(0)
	return []launchTimeoutCase{
		{name: "inherit", expected: termprobe.EffectiveTimeouts{Text: 11 * time.Millisecond, Idle: 12 * time.Millisecond, Command: 13 * time.Millisecond, Exit: 14 * time.Millisecond, Ready: 15 * time.Millisecond}},
		{name: "partial override", options: termprobe.Timeouts{Text: &zero, Ready: termprobe.Ptr(42 * time.Millisecond)}, expected: termprobe.EffectiveTimeouts{Idle: 12 * time.Millisecond, Command: 13 * time.Millisecond, Exit: 14 * time.Millisecond, Ready: 42 * time.Millisecond}},
		{name: "explicit zero", options: termprobe.Timeouts{Text: &zero, Idle: &zero, Command: &zero, Exit: &zero, Ready: &zero}},
	}
}

func TestClientTimeoutDefaultsReachLaunch(t *testing.T) {
	for _, launch := range []string{"open", "run"} {
		for _, testCase := range launchTimeoutCases() {
			t.Run(launch+"/"+testCase.name, func(t *testing.T) { verifyLaunchTimeouts(t, launch, testCase) })
		}
	}
}

func verifyLaunchTimeouts(t *testing.T, launch string, testCase launchTimeoutCase) {
	t.Helper()
	terminal := newClient(t, termprobe.ClientOptions{Timeouts: clientLaunchTimeouts()})
	options := termprobe.SpawnOptions{WaitReady: termprobe.Ptr(false), Timeouts: testCase.options}
	if launch == "run" {
