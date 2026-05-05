from __future__ import annotations

import asyncio
import atexit
import re
import threading
from contextlib import asynccontextmanager
from dataclasses import dataclass, replace
from typing import (
    Any,
    AsyncIterator,
    Dict,
    Iterable,
    Optional,
    Sequence,
    Set,
)

from ._config import IS_MACOS, IS_WINDOWS
from ._ephemeral import unique_session
from .client import TuiTest
from .errors import UsageError
from .types import AutomaticRecording, Backend, Profile, Timeouts, TraceOptions

__all__ = [
    "TerminalOptions",
    "DEFAULT_SHELL",
    "create_terminal",
    "terminal",
    "set_terminal_defaults",
    "reset_terminal_defaults",
    "track_terminal",
    "untrack_terminal",
    "tracked_count",
    "close_all_tracked",
    "terminal_snapshot",
]

DEFAULT_SHELL = "powershell" if IS_WINDOWS else "zsh" if IS_MACOS else "bash"


@dataclass
class TerminalOptions:
    backend: Optional[Backend] = None
    shell: Optional[str] = None
    program: Optional[Sequence[str]] = None
    cols: Optional[int] = None
    rows: Optional[int] = None
    cwd: Optional[str] = None
    env: Optional[Any] = None
    session: Optional[str] = None
    prefix: Optional[str] = None
    retries: Optional[int] = None
    wait_ready: Optional[bool] = None
    timeouts: Optional[Timeouts] = None
    profile: Optional[Profile] = None
    screen_history_limit: Optional[int] = None
    artifacts: Optional[Dict[str, Any]] = None
    recording: Optional[AutomaticRecording] = None
    trace: Optional[TraceOptions] = None


_DEFAULTABLE = frozenset(TerminalOptions.__dataclass_fields__)
_defaults = TerminalOptions()
_defaults_lock = threading.Lock()


def set_terminal_defaults(**values: Any) -> None:
    unknown = sorted(set(values) - _DEFAULTABLE)
    if unknown:
        raise TypeError(
            "unknown terminal option {}".format(", ".join(repr(k) for k in unknown))
        )
    global _defaults
    with _defaults_lock:
        _defaults = replace(_defaults, **values)


def get_terminal_defaults() -> TerminalOptions:
    return _defaults


def reset_terminal_defaults() -> None:
    global _defaults
    with _defaults_lock:
        _defaults = TerminalOptions()


_tracked = set()  # type: Set[TuiTest]
_tracked_lock = threading.Lock()
_safety_net_installed = False


def _install_safety_net() -> None:
    global _safety_net_installed
