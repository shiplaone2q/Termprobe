from __future__ import annotations

from typing import Mapping, Optional

from .diagnostics import FailureArtifactRef, FailureDetails


class TuiTestError(Exception):
    kind: str = "internal"
    exit_code: int = 5

    def __init__(
        self,
        message: str,
        *,
        details: Optional[FailureDetails] = None,
        artifact: Optional[FailureArtifactRef] = None,
    ) -> None:
        super().__init__(message)
        self.message = message
        self.details = details
        self.artifact = artifact


class ExpectationError(TuiTestError):
    kind = "assertion"
    exit_code = 1
