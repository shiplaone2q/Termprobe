from __future__ import annotations

import collections.abc
import dataclasses
import os
import sys
from typing import Any, Dict, Mapping, Optional

VERSION = "0.1.1"

DEFAULT_COLS = 80
DEFAULT_ROWS = 30

IS_WINDOWS = sys.platform == "win32"
IS_MACOS = sys.platform == "darwin"


def resolve_session(session: Optional[str]) -> str:
    return session or os.environ.get("TUI_TEST_SESSION") or "default"


_TIMEOUT_CLASSES = ("text", "idle", "command", "exit", "ready")
_BACKENDS = ("alacritty", "ghostty", "rio", "xtermjs")
_TRACE_MODES = ("off", "on-failure", "on")
_PROFILE_FIELDS = frozenset(("scrollback", "colors"))
_COLOR_FIELDS = frozenset(
    (
        "foreground",
        "background",
        "cursor",
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "white",
        "bright_black",
        "bright_red",
        "bright_green",
        "bright_yellow",
        "bright_blue",
        "bright_magenta",
        "bright_cyan",
        "bright_white",
    )
)


def resolve_timeout(
    class_name: str,
    *,
    call: Optional[int] = None,
    timeouts: Optional[Mapping[str, Optional[int]]] = None,
) -> Optional[int]:
    if call is not None:
        return call
    if timeouts is not None:
        return timeouts.get(class_name)
    return None


