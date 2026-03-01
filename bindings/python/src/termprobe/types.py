from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Literal, Optional, TypedDict, Union

Color = Union[str, int]
Backend = Literal["alacritty", "ghostty", "rio", "xtermjs"]
MouseButton = Literal["left", "middle", "right"]
#: ``"none"`` is a value, not an absence: an un-underlined cell reports it.
UnderlineStyle = Literal["none", "single", "double", "curly", "dotted", "dashed"]
RecordingFormat = Literal["apng", "gif", "mp4", "cast"]
TraceMode = Literal["off", "on-failure", "on"]
LocatorDirection = Literal["within", "after", "before"]


class OpenResult(TypedDict):
    shell_pid: Optional[int]
    session: str
    ready: bool
    recording: str


class Viewport(TypedDict):
    offset: int
    history: int


class Cursor(TypedDict):
    x: int
    y: int
    visible: bool
    shape: str
    color: str


@dataclass
class Colors:
    foreground: Optional[str] = None
    background: Optional[str] = None
    cursor: Optional[str] = None
    black: Optional[str] = None
    red: Optional[str] = None
    green: Optional[str] = None
    yellow: Optional[str] = None
    blue: Optional[str] = None
    magenta: Optional[str] = None
    cyan: Optional[str] = None
    white: Optional[str] = None
    bright_black: Optional[str] = None
    bright_red: Optional[str] = None
    bright_green: Optional[str] = None
    bright_yellow: Optional[str] = None
    bright_blue: Optional[str] = None
    bright_magenta: Optional[str] = None
    bright_cyan: Optional[str] = None
    bright_white: Optional[str] = None


@dataclass
