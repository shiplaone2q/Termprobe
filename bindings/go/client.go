package termprobe

import (
	"crypto/rand"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// Client is a named handle. Handles with the same name share a session and can
// be reused after that session closes. Its synchronous methods may run in goroutines.
type Client struct {
	session  string
	options  ClientOptions
	runtime  *nativeRuntime
	Keyboard *Keyboard
	Mouse    *Mouse
}

// New resolves an empty session from TERMPROBE__SESSION, then "default".
func New(session string, options ClientOptions) (*Client, error) {
	if session == "" {
		session = os.Getenv("TERMPROBE__SESSION")
	}
	if session == "" {
		session = "default"
	}
	if err := validateTimeouts(options.Timeouts); err != nil {
		return nil, err
	}
	if err := validateClientOptions(options); err != nil {
		return nil, err
	}
	options = cloneClientOptions(options)
	if options.Artifacts != nil {
		if options.Artifacts.OnFailure == "" {
			options.Artifacts.OnFailure = ArtifactAll
		}
		absoluteDirectory, err := filepath.Abs(options.Artifacts.Dir)
		if err != nil {
			return nil, &Error{Kind: UsageError, Message: "resolve artifact directory: " + err.Error()}
		}
		options.Artifacts.Dir = absoluteDirectory
	}
	if options.Trace != nil && options.Trace.Mode == "" {
		options.Trace.Mode = TraceOff
	}
	runtime, err := newNativeRuntime(session, options)
	if err != nil {
		return nil, err
	}
	client := &Client{session: session, options: options, runtime: runtime}
	client.Keyboard = &Keyboard{runtime: runtime}
	client.Mouse = &Mouse{runtime: runtime}
	return client, nil
}

func validateClientOptions(options ClientOptions) error {
	switch options.Backend {
	case "", Alacritty, Ghostty, Rio, XtermJS:
	default:
		return &Error{Kind: UsageError, Message: "unknown backend: " + string(options.Backend)}
	}
	if options.Recording != nil {
		switch options.Recording.Mode {
		case "", RecordingDisabled, RecordingOnFailure, RecordingAlways:
		default:
			return &Error{Kind: UsageError, Message: "unknown recording mode: " + string(options.Recording.Mode)}
		}
	}
	if options.Trace != nil {
		switch options.Trace.Mode {
		case "", TraceOff, TraceOnFailure, TraceOn:
		default:
			return &Error{Kind: UsageError, Message: "unknown trace mode: " + string(options.Trace.Mode)}
		}
	}
	if options.ScreenHistoryLimit != nil && *options.ScreenHistoryLimit > 50 {
		return &Error{Kind: UsageError, Message: "screen history limit must be at most 50"}
	}
	return validateArtifactOptions(options.Artifacts)
}
func validateArtifactOptions(options *ArtifactOptions) error {
	if options != nil {
		switch options.OnFailure {
		case "", ArtifactAll, ArtifactHTML, ArtifactText, ArtifactNone:
		default:
			return &Error{Kind: UsageError, Message: "unknown artifact mode: " + string(options.OnFailure)}
		}
		if options.Dir == "" {
			return &Error{Kind: UsageError, Message: "artifact directory must not be empty"}
		}
	}
	return nil
}

// Ephemeral creates a handle with a unique, filename-safe session name.
func Ephemeral(prefix string, options ClientOptions) (*Client, error) {
	if prefix == "" {
		prefix = "go"
	}
	prefix = strings.Map(func(character rune) rune {
		if character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' || character >= '0' && character <= '9' || character == '-' || character == '_' {
