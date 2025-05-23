package native

import (
	"encoding/json"
	"fmt"
	"time"
)

type Session struct {
	name      string
	recording *AutomaticRecording
}

func NewSession(name string, options SessionOptions) (*Session, error) {
	if err := checkNativeVersion(); err != nil {
		return nil, err
	}
	context := struct {
		Name                     string  `json:"name"`
		ArtifactDirectory        *string `json:"artifact_directory,omitempty"`
		ArtifactMode             *string `json:"artifact_mode,omitempty"`
		ArtifactIncludeRecording bool    `json:"artifact_include_recording,omitempty"`
		TraceMode                *string `json:"trace_mode,omitempty"`
		TraceDirectory           *string `json:"trace_directory,omitempty"`
		ScreenHistoryLimit       *uint16 `json:"screen_history_limit,omitempty"`
	}{Name: name, ScreenHistoryLimit: options.ScreenHistoryLimit}
	if options.Artifacts != nil {
		context.ArtifactDirectory = pointerTo(options.Artifacts.Directory)
		context.ArtifactMode = pointerTo(options.Artifacts.Mode)
		context.ArtifactIncludeRecording = options.Artifacts.IncludeRecording
	}
	if options.Trace != nil {
		context.TraceMode = pointerTo(options.Trace.Mode)
		if options.Trace.Directory != "" {
			context.TraceDirectory = pointerTo(options.Trace.Directory)
		}
	}
	encoded, err := json.Marshal(context)
	if err != nil {
		return nil, &Error{Kind: InternalError, Message: "encode native session context: " + err.Error()}
	}
	return &Session{name: string(encoded), recording: options.Recording}, nil
}

func checkNativeVersion() error {
	if err := loadNativeEngine(); err != nil {
		return &Error{Kind: InternalError, Message: err.Error()}
	}
	if version := nativeFunctions.AbiVersion(); version != abiVersion {
		return &Error{Kind: InternalError, Message: fmt.Sprintf("native ABI version %d is incompatible with required version %d", version, abiVersion)}
	}
	return nil
}

func Sessions() ([]string, error) {
	if err := checkNativeVersion(); err != nil {
		return nil, err
	}
	result := nativeFunctions.Sessions()
	defer nativeFunctions.ResultFree(result)
	if err := nativeError(result); err != nil {
		return nil, err
	}
	return nativeSessionNames(result), nil
}

func CloseAll() error {
	if err := checkNativeVersion(); err != nil {
		return err
	}
	result := nativeFunctions.CloseAll()
	defer nativeFunctions.ResultFree(result)
	return nativeError(result)
}

func Recording(session string) (string, error) {
	if err := checkNativeVersion(); err != nil {
		return "", err
	}
	memory := nativeMemory{}
	defer memory.release()
	result := nativeFunctions.Recording(memory.text(session))
	defer nativeFunctions.ResultFree(result)
	if err := nativeError(result); err != nil {
		return "", err
	}
	return nativeText(result.text), nil
}

func (runtime *Session) Open(options OpenOptions) (OpenResult, error) {
	memory := nativeMemory{}
	defer memory.release()
	converted, err := memory.openOptions(options, runtime.recording)
	if err != nil {
		return OpenResult{}, err
	}
	memory.pinner.Pin(&converted)
	result := nativeFunctions.Open(memory.text(runtime.name), &converted)
	defer nativeFunctions.ResultFree(result)
	return readOpenResult(result)
}

func (runtime *Session) Run(program string, args []string, options SpawnOptions) (OpenResult, error) {
	memory := nativeMemory{}
	defer memory.release()
	converted, err := memory.openOptions(OpenOptions{SpawnOptions: options}, runtime.recording)
	if err != nil {
		return OpenResult{}, err
	}
	arguments, length := memory.strings(args)
	memory.pinner.Pin(&converted)
	result := nativeFunctions.Run(memory.text(runtime.name), &converted, memory.text(program), arguments, length)
	defer nativeFunctions.ResultFree(result)
	return readOpenResult(result)
}

func (runtime *Session) State() (State, error) {
	memory := nativeMemory{}
	defer memory.release()
	result := nativeFunctions.State(memory.text(runtime.name))
	defer nativeFunctions.ResultFree(result)
	if err := nativeError(result); err != nil {
		return State{}, err
	}
	return nativeState(result.state), nil
}

func (runtime *Session) Text(full bool) (string, error) {
	memory := nativeMemory{}
	defer memory.release()
	result := nativeFunctions.Text(memory.text(runtime.name), full)
	defer nativeFunctions.ResultFree(result)
	if err := nativeError(result); err != nil {
		return "", err
	}
	return nativeText(result.text), nil
}

func (runtime *Session) Cells(column, row, width, height uint16) ([]Cell, error) {
	memory := nativeMemory{}
	defer memory.release()
	result := nativeFunctions.Cells(memory.text(runtime.name), column, row, width, height)
	defer nativeFunctions.ResultFree(result)
	if err := nativeError(result); err != nil {
		return nil, err
	}
	return nativeCells(result), nil
}

func (runtime *Session) Resize(cols, rows uint16) error {
	memory := nativeMemory{}
	defer memory.release()
	result := nativeFunctions.Resize(memory.text(runtime.name), cols, rows)
	defer nativeFunctions.ResultFree(result)
	return nativeError(result)
}

func (runtime *Session) Submit(text *string) error {
	memory := nativeMemory{}
	defer memory.release()
	result := nativeFunctions.Submit(memory.text(runtime.name), memory.optionalText(text))
	defer nativeFunctions.ResultFree(result)
	return nativeError(result)
}

func (runtime *Session) Key(keys []string, action uint32) error {
	memory := nativeMemory{}
	defer memory.release()
	values, length := memory.strings(keys)
	result := nativeFunctions.Key(memory.text(runtime.name), values, length, action)
	defer nativeFunctions.ResultFree(result)
	return nativeError(result)
}

func (runtime *Session) GetCursor() (Cursor, error) {
	memory := nativeMemory{}
	defer memory.release()
	result := nativeFunctions.GetCursor(memory.text(runtime.name))
