package termprobe

import (
	"encoding/json"
	"errors"
	"time"

	"github.com/microsoft/termprobe/bindings/go/internal/native"
)

type nativeRuntime struct{ session *native.Session }

func newNativeRuntime(name string, options ClientOptions) (*nativeRuntime, error) {
	var nativeRecording *native.AutomaticRecording
	if options.Recording != nil {
		nativeRecording = &native.AutomaticRecording{Mode: native.RecordingMode(options.Recording.Mode), Directory: options.Recording.Directory}
	}
	var artifacts *native.ArtifactOptions
	if options.Artifacts != nil {
		artifacts = &native.ArtifactOptions{Directory: options.Artifacts.Dir, Mode: string(options.Artifacts.OnFailure), IncludeRecording: options.Artifacts.IncludeRecording}
	}
	var trace *native.TraceOptions
	if options.Trace != nil {
		trace = &native.TraceOptions{Mode: string(options.Trace.Mode), Directory: options.Trace.Directory}
	}
	session, err := native.NewSession(name, native.SessionOptions{Recording: nativeRecording, Artifacts: artifacts, Trace: trace, ScreenHistoryLimit: options.ScreenHistoryLimit})
	if err != nil {
		return nil, engineError(err)
	}
	return &nativeRuntime{session: session}, nil
}
func engineError(err error) error {
	if err == nil {
		return nil
	}
	var failure *native.Error
	if errors.As(err, &failure) {
		converted := &Error{Kind: ErrorKind(failure.Kind), Message: failure.Message}
		if len(failure.DetailsJSON) != 0 && json.Unmarshal(failure.DetailsJSON, &converted.Details) != nil {
			return &Error{Kind: InternalError, Message: "decode native failure details"}
		}
		if len(failure.ArtifactJSON) != 0 && json.Unmarshal(failure.ArtifactJSON, &converted.Artifact) != nil {
			return &Error{Kind: InternalError, Message: "decode native failure artifact"}
		}
		return converted
	}
	return &Error{Kind: InternalError, Message: err.Error()}
}
func engineTimeoutMilliseconds(timeout time.Duration) (uint64, error) {
	value, err := native.Milliseconds(timeout)
	return value, engineError(err)
}
func nativeSessions() ([]string, error) {
	value, err := native.Sessions()
	return value, engineError(err)
}
