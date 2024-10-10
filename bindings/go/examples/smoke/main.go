// The smoke consumer runs itself in child mode, so it needs no external shell.
package main

import (
	"bufio"
	"fmt"
	"os"
	"time"

	termprobe "github.com/microsoft/termprobe/bindings/go"
)

func main() {
	if len(os.Args) > 1 && os.Args[1] == "--child" {
		fmt.Println("go-binding-ready")
		scanner := bufio.NewScanner(os.Stdin)
		for scanner.Scan() {
			if scanner.Text() == "quit" {
				return
			}
			fmt.Println("echo:" + scanner.Text())
		}
		return
	}
	if err := smoke(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	source := "bundled"
	if os.Getenv("TERMPROBE__NATIVE_LIBRARY") != "" {
		source = "override"
	}
	fmt.Printf("Go binding smoke passed (%s engine)\n", source)
}

func smoke() error {
	terminal, err := termprobe.Ephemeral("smoke", termprobe.ClientOptions{Recording: &termprobe.AutomaticRecording{Mode: termprobe.RecordingDisabled}})
