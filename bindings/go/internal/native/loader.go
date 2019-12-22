package native

import (
	"errors"
	"fmt"
	"sync"

	"github.com/ebitengine/purego"
)

type nativeRegistration struct {
	symbol string
	target any
}
