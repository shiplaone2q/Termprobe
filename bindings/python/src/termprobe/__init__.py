from __future__ import annotations

from ._config import VERSION as __version__
from ._ephemeral import unique_session
from .client import Locator, TuiTest, close_all, get_recording, sessions
from .diagnostics import (
    FailureArtifactRef,
    FailureArtifactStatus,
    FailureDetails,
    FailureReason,
)
from .errors import (
    ExpectationError,
    InternalError,
    NoSessionError,
    TuiTestError,
    UsageError,
)
from .types import (
    AutomaticRecording,
