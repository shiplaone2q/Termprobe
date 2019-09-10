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

