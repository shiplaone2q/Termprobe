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
