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
