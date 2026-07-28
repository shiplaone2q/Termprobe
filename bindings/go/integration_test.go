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
	if err = terminal.GetByText("go-ready", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{Timeout: termprobe.Ptr(10 * time.Second)}); err != nil {
		t.Fatal(err)
	}
}
func requireNoError(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatal(err)
	}
}
func requireKind(t *testing.T, err error, kind termprobe.ErrorKind) {
	t.Helper()
	var failure *termprobe.Error
	if !errors.As(err, &failure) || failure.Kind != kind {
		t.Fatalf("expected %s error, got %v", kind, err)
	}
}

func TestLocatorsPreserveSelection(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	items := terminal.GetByText("item", termprobe.TextSelectorOptions{})
	count, err := items.Count()
	requireNoError(t, err)
	if count != 2 {
		t.Fatalf("item count=%d", count)
	}
	_, err = items.Location()
	requireKind(t, err, termprobe.AssertionError)
	first, err := items.First().Location()
	requireNoError(t, err)
	last, err := items.Last().Location()
	requireNoError(t, err)
	if first.Start.Column >= last.Start.Column {
		t.Fatalf("unordered locations: %+v %+v", first, last)
	}
	requireNoError(t, items.Highlight(termprobe.WaitOptions{}))
	requireNoError(t, items.First().Click(termprobe.LocatorClickOptions{}))
	requireNoError(t, terminal.GetByText("absent", termprobe.TextSelectorOptions{}).Wait(termprobe.LocatorWaitOptions{State: termprobe.Hidden, Timeout: termprobe.Ptr(time.Duration(0))}))
}

func TestLocatorEnumerationAndRelativeQueries(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	items := terminal.GetByText("item", termprobe.TextSelectorOptions{})
	all, err := items.All()
	requireNoError(t, err)
	for index, locator := range all {
		actual, locationErr := locator.Location()
		requireNoError(t, locationErr)
		selected, selectedErr := items.Nth(uint32(index)).Location()
		requireNoError(t, selectedErr)
		if actual.Start != selected.Start {
			t.Fatalf("all[%d]=%+v; nth=%+v", index, actual, selected)
		}
	}
	count, err := items.Count()
	requireNoError(t, err)
	if count != 2 {
		t.Fatal("selection mutated original locator")
	}
	requireNoError(t, terminal.GetByText("item item", termprobe.TextSelectorOptions{}).GetByText("item", termprobe.TextSelectorOptions{}).Nth(1).Expect(termprobe.LocatorExpectOptions{}))
	requireNoError(t, items.First().GetByText("item", termprobe.TextSelectorOptions{Direction: termprobe.After}).Expect(termprobe.LocatorExpectOptions{}))
}

func TestStylesAndWideCellCoordinates(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	bold := true
	styled := terminal.GetByStyle(termprobe.TextStyle{Bold: &bold}, termprobe.StyleSelectorOptions{})
	bold = false
	requireNoError(t, styled.GetByText("styled", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{}))
	requireNoError(t, terminal.GetByText("plain", termprobe.TextSelectorOptions{}).GetByStyle(termprobe.TextStyle{Bold: termprobe.Ptr(false)}, termprobe.StyleSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{}))
	wide, err := terminal.GetByText("界", termprobe.TextSelectorOptions{}).Location()
	requireNoError(t, err)
	if wide.Start.Column != 0 || wide.Start.Row != 3 {
		t.Fatalf("unexpected wide cell coordinates: %+v", wide.Start)
	}
	cells, err := terminal.Cells(0, 3, 2, 1)
	requireNoError(t, err)
	if len(cells) != 2 || cells[0].Char != "界" || cells[1].Char != "" {
		t.Fatalf("wide cells=%+v", cells)
	}
}

func TestLinkAndComposableLocators(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	row := terminal.GetByText("linked docs", termprobe.TextSelectorOptions{})
	link := terminal.GetByLink("https://example.test/docs", termprobe.LinkSelectorOptions{})
	intersection, err := row.And(link)
	requireNoError(t, err)
	match, err := intersection.Location()
	requireNoError(t, err)
	if match.Text != "linked docs" {
		t.Fatalf("intersection=%+v", match)
	}
	union, err := terminal.GetByText("absent", termprobe.TextSelectorOptions{}).Or(link)
	requireNoError(t, err)
	requireNoError(t, union.Expect(termprobe.LocatorExpectOptions{}))
	filtered, err := row.Filter(termprobe.LocatorFilterOptions{Has: link})
	requireNoError(t, err)
	requireNoError(t, filtered.Expect(termprobe.LocatorExpectOptions{}))
	linked, err := link.Location()
	requireNoError(t, err)
	cells, err := terminal.Cells(uint16(linked.Start.Column), uint16(linked.Start.Row), 1, 1)
	requireNoError(t, err)
	if len(cells) != 1 || cells[0].Link != "https://example.test/docs" {
		t.Fatalf("linked cell=%+v", cells)
	}
}

func TestInputAndState(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	state, err := terminal.State()
	requireNoError(t, err)
	if state.Cols != 80 || state.Rows != 30 {
		t.Fatalf("default size=%dx%d", state.Cols, state.Rows)
	}
	if state.Cursor.Shape == "" || state.Cursor.Color == "" || state.Colors.Foreground == "" || state.Colors.Background == "" || state.Colors.Cursor == "" || state.Modes == nil || state.MouseMode == "" {
		t.Fatalf("incomplete rich state=%+v", state)
	}
	requireNoError(t, terminal.Resize(90, 32))
	size, err := terminal.GetSize()
	requireNoError(t, err)
	if size.Cols != 90 || size.Rows != 32 {
		t.Fatalf("resized size=%+v", size)
	}
	_, err = terminal.GetCursor()
	requireNoError(t, err)
	_, err = terminal.GetCommand()
	requireNoError(t, err)
	_, err = terminal.GetOutput()
	requireNoError(t, err)
	_, err = terminal.GetExitCode()
	requireNoError(t, err)
	_, err = terminal.GetCwd()
	requireNoError(t, err)
	requireNoError(t, terminal.Type("hello"))
	requireNoError(t, terminal.Press("Enter"))
	requireNoError(t, terminal.GetByText("received:hello", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{}))
	requireNoError(t, terminal.Submit(termprobe.Ptr("quit")))
	requireNoError(t, terminal.WaitExit(termprobe.WaitOptions{Timeout: termprobe.Ptr(10 * time.Second)}))
}

func TestTerminalEvents(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	requireNoError(t, terminal.Submit(termprobe.Ptr("events")))
	requireNoError(t, terminal.WaitTitle("Go title", termprobe.TitleOptions{}))
	requireNoError(t, terminal.ExpectTitle("^Go", termprobe.TitleOptions{Regex: true}))
	requireNoError(t, terminal.WaitClipboard(termprobe.ClipboardWaitOptions{Text: termprobe.Ptr("go-clipboard")}))
	title, err := terminal.GetTitle()
	requireNoError(t, err)
	if title == nil || *title != "Go title" {
		t.Fatalf("title=%v", title)
	}
	clipboard, err := terminal.GetClipboard()
	requireNoError(t, err)
	if clipboard != "go-clipboard" {
		t.Fatalf("clipboard=%q", clipboard)
	}
	requireNoError(t, terminal.ExpectBellCount(1, termprobe.WaitOptions{}))
	events, err := terminal.GetBellEvents()
	requireNoError(t, err)
	if len(events) != 1 || events[0].Sequence != 1 {
		t.Fatalf("bell events=%+v", events)
	}
	count, err := terminal.GetBellCount()
	requireNoError(t, err)
	if count != 1 {
		t.Fatalf("bell count=%d", count)
	}
}

func TestScreenshotsAndSnapshots(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	text, err := terminal.Screenshot("", termprobe.ScreenshotOptions{})
	requireNoError(t, err)
	if !strings.Contains(text, "go-ready") {
		t.Fatalf("screenshot omitted terminal text: %.100s", text)
	}
	directory := t.TempDir()
	path, err := terminal.Screenshot(filepath.Join(directory, "terminal.svg"), termprobe.ScreenshotOptions{})
	requireNoError(t, err)
	if _, statErr := os.Stat(path); statErr != nil {
		t.Fatal(statErr)
	}
	// Raster tests use bundled font coverage rather than host CJK fonts.
	requireNoError(t, terminal.Submit(termprobe.Ptr("raster")))
	requireNoError(t, terminal.GetByText("go-raster-ready", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{}))
	transparent, err := terminal.Screenshot(filepath.Join(directory, "transparent.png"), termprobe.ScreenshotOptions{Transparent: true})
	requireNoError(t, err)
	if _, statErr := os.Stat(transparent); statErr != nil {
		t.Fatal(statErr)
	}
	data, err := os.ReadFile(transparent)
	requireNoError(t, err)
	baseline, err := png.DecodeConfig(bytes.NewReader(data))
	requireNoError(t, err)
	for index, options := range []termprobe.ScreenshotOptions{
		{Zoom: termprobe.Ptr(2.0), Transparent: true},
		{Zoom: termprobe.Ptr(2.0), Background: "#123456"},
	} {
		path, err := terminal.Screenshot(filepath.Join(directory, fmt.Sprintf("scaled-%d.png", index)), options)
		requireNoError(t, err)
		data, err := os.ReadFile(path)
		requireNoError(t, err)
		scaled, err := png.DecodeConfig(bytes.NewReader(data))
		requireNoError(t, err)
		if scaled.Width != 2*baseline.Width || scaled.Height != 2*baseline.Height {
			t.Fatalf("scaled screenshot=%dx%d, baseline=%dx%d", scaled.Width, scaled.Height, baseline.Width, baseline.Height)
		}
	}
	_, err = terminal.Screenshot(filepath.Join(directory, "conflict.png"), termprobe.ScreenshotOptions{Background: "#000000", Transparent: true})
	requireKind(t, err, termprobe.UsageError)
	snapshot, err := terminal.ExpectSnapshot("terminal", termprobe.SnapshotOptions{Cwd: directory, Update: true})
	requireNoError(t, err)
	if snapshot != termprobe.SnapshotWritten && snapshot != termprobe.SnapshotUpdated {
		t.Fatalf("snapshot=%s", snapshot)
	}
	_, err = terminal.ExpectSnapshot("terminal", termprobe.SnapshotOptions{Cwd: directory, IncludeStyle: true, Update: true})
	requireNoError(t, err)
	requireNoError(t, terminal.Submit(termprobe.Ptr("quit")))
	requireNoError(t, terminal.WaitExit(termprobe.WaitOptions{Timeout: termprobe.Ptr(10 * time.Second)}))
}

func TestRestartAndFailureAwareTraceClose(t *testing.T) {
	traceDirectory := t.TempDir()
	terminal := newClient(t, termprobe.ClientOptions{Trace: &termprobe.TraceOptions{Mode: termprobe.TraceOn, Directory: traceDirectory}})
	runTerminal(t, terminal, termprobe.SpawnOptions{})
	result, err := terminal.Restart(termprobe.RestartOptions{GracefulTimeout: termprobe.Ptr(2 * time.Second)})
	requireNoError(t, err)
	if result.Session != terminal.Session() {
		t.Fatalf("restart result=%+v", result)
	}
	requireNoError(t, terminal.GetByText("go-ready", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{Timeout: termprobe.Ptr(10 * time.Second)}))
	requireNoError(t, terminal.Close(termprobe.CloseOptions{Failed: termprobe.Ptr(true)}))
	entries, err := os.ReadDir(traceDirectory)
	requireNoError(t, err)
	if len(entries) == 0 {
		t.Fatal("failure-aware close did not retain a trace")
	}
}

func TestScreenHistoryLimitValidation(t *testing.T) {
	_, err := termprobe.New("history", termprobe.ClientOptions{ScreenHistoryLimit: termprobe.Ptr(uint16(51))})
	requireKind(t, err, termprobe.UsageError)
}

func TestNamedHandlesReopen(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	peer, err := termprobe.New(terminal.Session(), termprobe.ClientOptions{})
	requireNoError(t, err)
	names, err := termprobe.Sessions()
	requireNoError(t, err)
	if !slices.Contains(names, terminal.Session()) {
		t.Fatalf("missing session in %v", names)
	}
	requireNoError(t, terminal.Close())
	_, err = peer.Text(termprobe.TextOptions{})
	requireKind(t, err, termprobe.NoSessionError)
	runTerminal(t, terminal, termprobe.SpawnOptions{})
	requireNoError(t, peer.GetByText("go-ready", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{}))
}

func TestCloseInterruptsPendingWait(t *testing.T) {
	t.Skip("Known upstream limitation accepted for this binding: named Close blocks behind pending waits; https://github.com/microsoft/termprobe/issues/207")
	terminal := newTerminal(t, termprobe.ClientOptions{})
	verifyWaitInterruption(t, terminal, func() error { return terminal.Close() })
}

func TestCloseAllInterruptsPendingWait(t *testing.T) {
	terminal := newTerminal(t, termprobe.ClientOptions{})
	verifyWaitInterruption(t, terminal, termprobe.CloseAll)
}

func verifyWaitInterruption(t *testing.T, terminal *termprobe.Client, closeSession func() error) {
	t.Helper()
	started := make(chan struct{})
	waitResult := make(chan error, 1)
	go func() {
		close(started)
		waitResult <- terminal.GetByText("never-visible", termprobe.TextSelectorOptions{}).Wait(termprobe.LocatorWaitOptions{Timeout: termprobe.Ptr(5 * time.Second)})
	}()
	<-started
	select {
	case err := <-waitResult:
		t.Fatalf("wait returned before close: %v", err)
	case <-time.After(100 * time.Millisecond):
	}
	closeResult := make(chan error, 1)
	go func() { closeResult <- closeSession() }()
	select {
	case err := <-closeResult:
		requireNoError(t, err)
	case <-time.After(2 * time.Second):
		t.Fatal("close blocked behind the pending wait")
	}
	select {
	case err := <-waitResult:
		if err == nil {
			t.Fatal("interrupted wait unexpectedly succeeded")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("wait did not return after close")
	}
}

func TestTimeoutOptionsAndTypedFailures(t *testing.T) {
	_, err := termprobe.New("", termprobe.ClientOptions{Timeouts: termprobe.Timeouts{Text: termprobe.Ptr(-time.Nanosecond)}})
	requireKind(t, err, termprobe.UsageError)
	terminal := newTerminal(t, termprobe.ClientOptions{Timeouts: termprobe.Timeouts{Text: termprobe.Ptr(time.Duration(0))}, Artifacts: &termprobe.ArtifactOptions{Dir: t.TempDir(), OnFailure: termprobe.ArtifactAll}})
	requireNoError(t, terminal.Type("typed input"))
	err = terminal.GetByText("missing", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{})
	requireKind(t, err, termprobe.AssertionError)
	var failure *termprobe.Error
	if !errors.As(err, &failure) || failure.Details == nil || failure.Artifact == nil || failure.Artifact.ScreenSVG == nil {
		t.Fatalf("missing diagnostic/artifact: %#v", failure)
	}
	if failure.Details.Operation != "locator.expect" {
		t.Fatalf("failure operation=%q", failure.Details.Operation)
	}
	_, err = terminal.Screenshot("", termprobe.ScreenshotOptions{Zoom: termprobe.Ptr(2.0)})
	requireKind(t, err, termprobe.UsageError)
	requireNoError(t, terminal.Close())
	runTerminal(t, terminal, termprobe.SpawnOptions{Timeouts: termprobe.Timeouts{Text: termprobe.Ptr(1500 * time.Microsecond), Idle: termprobe.Ptr(time.Duration(0))}})
	state, err := terminal.State()
	requireNoError(t, err)
	if state.Timeouts.Text != 2*time.Millisecond || state.Timeouts.Idle != 0 {
		t.Fatalf("effective timeouts=%+v", state.Timeouts)
	}
}

func TestPublicOperationNamesReachDiagnostics(t *testing.T) {
	traceDirectory := t.TempDir()
	terminal := newTerminal(t, termprobe.ClientOptions{
		Timeouts: termprobe.Timeouts{Text: termprobe.Ptr(time.Duration(0))},
		Trace:    &termprobe.TraceOptions{Mode: termprobe.TraceOn, Directory: traceDirectory},
	})
	requireNoError(t, terminal.Type("typed input"))
	err := terminal.GetByText("missing", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{})
	requireKind(t, err, termprobe.AssertionError)
	var failure *termprobe.Error
	if !errors.As(err, &failure) || failure.Details == nil || failure.Details.Operation != "locator.expect" {
		t.Fatalf("failure details=%#v", failure)
	}
	requireNoError(t, terminal.Close(termprobe.CloseOptions{Failed: termprobe.Ptr(true)}))

	manifests, err := filepath.Glob(filepath.Join(traceDirectory, "*", "trace.json"))
	requireNoError(t, err)
	if len(manifests) != 1 {
		t.Fatalf("trace manifests=%v", manifests)
	}
	manifestData, err := os.ReadFile(manifests[0])
	requireNoError(t, err)
	var manifest struct {
		RecentOperations []struct {
			Name string `json:"name"`
		} `json:"recent_operations"`
	}
	requireNoError(t, json.Unmarshal(manifestData, &manifest))
	if !slices.ContainsFunc(manifest.RecentOperations, func(operation struct {
		Name string `json:"name"`
	}) bool {
		return operation.Name == "type"
	}) {
		t.Fatalf("recent operations=%+v", manifest.RecentOperations)
	}
}

func TestDefaultRecordingIsDisabled(t *testing.T) {
	terminal, err := termprobe.Ephemeral(t.Name(), termprobe.ClientOptions{})
	requireNoError(t, err)
	t.Cleanup(func() { _ = terminal.Close() })
	executable, err := os.Executable()
	requireNoError(t, err)
	result, err := terminal.Run(executable, []string{"-test.run=^TestTerminalProcess$", "--", "--tui-go-child"}, termprobe.SpawnOptions{WaitReady: termprobe.Ptr(false)})
	requireNoError(t, err)
	if result.Recording != "" {
		t.Fatalf("default recording path=%q", result.Recording)
	}
}

func TestRecordingAndCloseAll(t *testing.T) {
	directory := t.TempDir()
	terminal := newTerminal(t, termprobe.ClientOptions{Recording: &termprobe.AutomaticRecording{Mode: termprobe.RecordingAlways, Directory: directory}})
	recording, err := termprobe.Recording(terminal.Session())
	requireNoError(t, err)
	if !strings.Contains(recording, `"version":2`) {
		t.Fatal("missing automatic recording content")
	}
	requireNoError(t, terminal.StartRecording(filepath.Join(directory, "explicit.cast"), termprobe.RecordingOptions{Format: termprobe.Cast}))
	requireNoError(t, terminal.Submit(termprobe.Ptr("recorded")))
	requireNoError(t, terminal.GetByText("received:recorded", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{}))
	explicit, err := terminal.StopRecording()
	requireNoError(t, err)
	if _, statErr := os.Stat(explicit); statErr != nil {
		t.Fatal(statErr)
	}
	requireNoError(t, termprobe.CloseAll())
	retained, err := termprobe.Recording(terminal.Session())
	requireNoError(t, err)
	if !strings.Contains(retained, "recorded") {
		t.Fatalf("retained recording omitted terminal output: %q", retained)
	}
}
