package native

import (
	"encoding/json"
	"time"
)

func nativeDuration(value *time.Duration) (abiOptionalU64, error) {
	if value == nil {
		return abiOptionalU64{}, nil
	}
	milliseconds, err := Milliseconds(*value)
	if err != nil {
		return abiOptionalU64{}, err
	}
	return nativeUint(&milliseconds), nil
}

func nativeTimeouts(timeouts Timeouts) (abiTimeouts, error) {
	result := abiTimeouts{}
	targets := []*abiOptionalU64{&result.text, &result.idle, &result.command, &result.exit, &result.ready}
	for index, value := range []*time.Duration{timeouts.Text, timeouts.Idle, timeouts.Command, timeouts.Exit, timeouts.Ready} {
		converted, err := nativeDuration(value)
		if err != nil {
			return result, err
		}
		*targets[index] = converted
	}
	return result, nil
}

func (memory *nativeMemory) colors(colors Colors) (*abiPair, uintptr) {
	values := map[string]string{
		"foreground": colors.Foreground, "background": colors.Background, "cursor": colors.Cursor,
		"black": colors.Black, "red": colors.Red, "green": colors.Green, "yellow": colors.Yellow, "blue": colors.Blue, "magenta": colors.Magenta, "cyan": colors.Cyan, "white": colors.White,
		"bright_black": colors.BrightBlack, "bright_red": colors.BrightRed, "bright_green": colors.BrightGreen, "bright_yellow": colors.BrightYellow, "bright_blue": colors.BrightBlue, "bright_magenta": colors.BrightMagenta, "bright_cyan": colors.BrightCyan, "bright_white": colors.BrightWhite,
	}
	for key, value := range values {
		if value == "" {
			delete(values, key)
		}
	}
	return memory.pairs(values)
}

func (memory *nativeMemory) openOptions(options OpenOptions, recording *AutomaticRecording) (abiOpenOptions, error) {
	timeouts, err := nativeTimeouts(options.Timeouts)
	if err != nil {
		return abiOpenOptions{}, err
	}
	result := abiOpenOptions{backend: memory.nonemptyText(string(options.Backend)), shell: memory.nonemptyText(string(options.Shell)), cols: nativeUint(options.Cols), rows: nativeUint(options.Rows), cwd: memory.nonemptyText(options.Cwd), waitReady: nativeBool(options.WaitReady), timeouts: timeouts}
	if options.Restart != nil {
		result.restart = *options.Restart
	}
	result.env, result.envLen = memory.pairs(options.Env)
	if options.Profile != nil {
		result.scrollback = nativeUint(options.Profile.Scrollback)
		result.colors, result.colorsLen = memory.colors(options.Profile.Colors)
	}
	if recording != nil {
		result.recordingMode = memory.nonemptyText(string(recording.Mode))
		result.recordingDirectory = memory.nonemptyText(recording.Directory)
	}
	return result, nil
}
