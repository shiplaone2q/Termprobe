package termprobe

import (
	"errors"
	"testing"
)

func TestNativeOptionPointersPreserveValidation(t *testing.T) {
	terminal, err := Ephemeral(t.Name(), ClientOptions{Recording: &AutomaticRecording{Mode: RecordingDisabled}})
	if err != nil {
