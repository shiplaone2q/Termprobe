// Package termprobe drives terminal programs through the in-process termprobe engine.
package termprobe

import "time"

// Ptr marks an option as explicitly supplied, including false and zero.
func Ptr[Value any](value Value) *Value { return &value }

type Backend string

const (
	Alacritty Backend = "alacritty"
	Ghostty   Backend = "ghostty"
	Rio       Backend = "rio"
	XtermJS   Backend = "xtermjs"
)

type Shell string

const (
	Bash       Shell = "bash"
	PowerShell Shell = "powershell"
	Pwsh       Shell = "pwsh"
	Cmd        Shell = "cmd"
	Fish       Shell = "fish"
	Zsh        Shell = "zsh"
	Xonsh      Shell = "xonsh"
	Elvish     Shell = "elvish"
	Nushell    Shell = "nushell"
)

type UnderlineStyle string

const (
	UnderlineNone   UnderlineStyle = "none"
	UnderlineSingle UnderlineStyle = "single"
	UnderlineDouble UnderlineStyle = "double"
	UnderlineCurly  UnderlineStyle = "curly"
	UnderlineDotted UnderlineStyle = "dotted"
	UnderlineDashed UnderlineStyle = "dashed"
)

type RecordingMode string

const (
	RecordingDisabled  RecordingMode = "disabled"
	RecordingOnFailure RecordingMode = "on-failure"
	RecordingAlways    RecordingMode = "always"
)

type RecordingFormat string

const (
	APNG RecordingFormat = "apng"
	GIF  RecordingFormat = "gif"
	MP4  RecordingFormat = "mp4"
	Cast RecordingFormat = "cast"
)

type MouseButton string

const (
	Left   MouseButton = "left"
	Middle MouseButton = "middle"
	Right  MouseButton = "right"
)

type Direction string

const (
	Within Direction = "within"
	After  Direction = "after"
