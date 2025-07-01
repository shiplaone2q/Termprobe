from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Mapping, Optional, Tuple


class FailureReason(str, Enum):
    COMPLETED = "completed"
    TEST_FAILED = "test_failed"
    TIMED_OUT = "timed_out"
    SESSION_EXITED = "session_exited"
    CANCELLED = "cancelled"
    LOCATOR_NO_MATCH = "locator_no_match"
    LOCATOR_AMBIGUOUS = "locator_ambiguous"
    UNEXPECTED_MATCH = "unexpected_match"
    MATCH_NOT_ACTIONABLE = "match_not_actionable"
    SCALAR_MISMATCH = "scalar_mismatch"
    SNAPSHOT_MISMATCH = "snapshot_mismatch"
    EMULATOR_FAULT = "emulator_fault"
    INTERNAL_FAILURE = "internal_failure"


class FailureArtifactStatus(str, Enum):
    WRITTEN = "written"
    PARTIAL = "partial"
    FAILED = "failed"


def _optional_string(value: object) -> Optional[str]:
    if value is not None and not isinstance(value, str):
        raise TypeError("optional artifact paths must be strings or null")
    return value


@dataclass(frozen=True)
class FailureLocation:
    row: int
    column: int


@dataclass(frozen=True)
class FailureCellMismatch:
    location: FailureLocation
    grapheme: str
    property: str
    operator: str
    expected: str
    actual: str
    reason: str
    resolved: Optional[str] = None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "FailureCellMismatch":
        return cls(
            location=FailureLocation(**value["location"]),
            grapheme=value["grapheme"],
            property=value["property"],
            operator=value["operator"],
            expected=value["expected"],
