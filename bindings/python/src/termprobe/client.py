from __future__ import annotations

import atexit
import copy
import json
import operator
import os
import re
import time
from dataclasses import asdict
from typing import (
    Any,
    Awaitable,
    Callable,
    Dict,
    Iterable,
    Literal,
    List,
    Mapping,
    Optional,
    Pattern,
    Tuple,
    TypeVar,
    Union,
)

from . import _config as cfg
from . import _ephemeral as ephemeral
from . import _native as native
from .errors import (
    ExpectationError,
    InternalError,
    NoSessionError,
    TuiTestError,
    UsageError,
    make_error,
)
from .types import (
    AutomaticRecording,
    TraceOptions,
    Backend,
    BellEvent,
    Cell,
    Cursor,
    Viewport,
    LocatorDirection,
    MouseButton,
    OpenResult,
    Profile,
    RecordingFormat,
    State,
    TextMatch,
    TextStyle,
    Timeouts,
)

_ERROR_JSON_ATTRIBUTE = "_tui_test_error_json"
_TIMEOUT_CLASSES = ("text", "idle", "command", "exit", "ready")
_ARTIFACT_MODES = ("all", "html", "text", "none")

_T = TypeVar("_T")


EnvLike = Union[Mapping[str, str], Iterable[Tuple[str, str]], None]
_Occurrence = Union[Literal["any", "unique", "first", "last"], int]


async def _await_native(awaitable: Awaitable[_T]) -> _T:
    try:
        return await awaitable
    except (
        native.NativeAssertionError,
        native.NativeUsageError,
        native.NativeNoSessionError,
        native.NativeInternalError,
    ) as error:
        raise _decode_native_error(error) from error


def _decode_native_error(error: Exception) -> TuiTestError:
    raw = getattr(error, _ERROR_JSON_ATTRIBUTE, None)
    if not isinstance(raw, str):
        return InternalError(
            "malformed native error envelope: expected a JSON string"
        )
    try:
        envelope = json.loads(raw)
        if not isinstance(envelope, Mapping):
            raise TypeError("expected an object")
        kind = envelope["kind"]
        if kind not in ("assertion", "usage", "no_session", "internal"):
            raise ValueError("invalid error kind")
        message = envelope["message"]
        if not isinstance(message, str):
            raise TypeError("message must be a string")
        details = envelope.get("details")
        artifact = envelope.get("artifact")
        if details is not None and not isinstance(details, Mapping):
            raise TypeError("details must be an object or null")
        if artifact is not None and not isinstance(artifact, Mapping):
            raise TypeError("artifact must be an object or null")
        return make_error(
            kind,
            message,
            details=details,
            artifact=artifact,
        )
    except (AttributeError, KeyError, TypeError, ValueError) as decode_error:
        return InternalError(
            "malformed native error envelope: {}".format(decode_error)
        )


def _atexit_close_all() -> None:
    try:
        native._close_all_blocking()
    except Exception:
        pass


atexit.register(_atexit_close_all)


def _env_pairs(env: EnvLike) -> List[Tuple[str, str]]:
    if env is None:
        return []
    items = env.items() if isinstance(env, Mapping) else env
    return [(str(key), str(value)) for key, value in items]


def _session_timeout_values(timeouts: object) -> Tuple[Optional[int], ...]:
    normalized = cfg.session_timeouts_payload(timeouts) or {}
    return tuple(normalized.get(class_name) for class_name in _TIMEOUT_CLASSES)


def _profile_values(
    profile: object,
) -> Tuple[Optional[int], List[Tuple[str, str]]]:
    normalized = cfg.normalize_profile(profile) or {}
    colors = normalized.get("colors") or {}
    return normalized.get("scrollback"), list(colors.items())


def _occurrence_fields(value: _Occurrence) -> Dict[str, object]:
    if isinstance(value, int) and not isinstance(value, bool):
        if value < 0:
            raise ValueError("occurrence index must be non-negative")
        return {"occurrence": "nth", "nth": value}
    if value not in ("any", "unique", "first", "last"):
        raise ValueError(
            "occurrence must be any, unique, first, last, or a non-negative index"
        )
    return {"occurrence": value, "nth": None}


def _text_stage_value(
    text: str,
    *,
    regex: bool,
    full: bool,
    whitespace: str,
    direction: LocatorDirection,
) -> Dict[str, object]:
    if direction not in ("within", "after", "before"):
        raise ValueError("locator direction must be within, after, or before")
    return {
        "kind": "text",
        "direction": direction,
        "text": text,
        "regex": regex,
        "full": full,
        "whitespace": whitespace,
        **_occurrence_fields("any"),
    }


def _text_query_value(
    text: str,
    *,
    regex: bool,
    full: bool,
    whitespace: str,
    direction: LocatorDirection,
    within: Optional["_LocatorQuery"],
) -> "_LocatorQuery":
    return _append_node(
        within,
        _text_stage_value(
            text,
            regex=regex,
            full=full,
            whitespace=whitespace,
            direction=direction,
        ),
        relative=True,
    )


def _style_query_value(
    style: TextStyle,
    *,
    full: bool,
    direction: LocatorDirection,
    within: Optional["_LocatorQuery"],
) -> "_LocatorQuery":
    if not isinstance(style, TextStyle):
        raise TypeError("style must be a TextStyle")
    style_value = asdict(style)
    if set(vars(style)) - set(TextStyle.__dataclass_fields__):
        raise ValueError("unknown style property")
    if not any(value is not None for value in style_value.values()):
        raise ValueError("get_by_style requires at least one style property")
    if direction not in ("within", "after", "before"):
        raise ValueError("locator direction must be within, after, or before")
    return _append_node(
        within,
        {
            "kind": "style",
            "direction": direction,
            "style": style_value,
            "full": full,
            **_occurrence_fields("any"),
        },
        relative=True,
    )


def _link_query_value(
    uri: str,
    *,
    full: bool,
    direction: LocatorDirection,
    within: Optional["_LocatorQuery"],
) -> "_LocatorQuery":
    if not isinstance(uri, str):
        raise TypeError("get_by_link requires a URI string")
    if direction not in ("within", "after", "before"):
        raise ValueError("locator direction must be within, after, or before")
    return _append_node(
        within,
        {
            "kind": "link",
            "link": uri,
            "full": full,
            "direction": direction,
            **_occurrence_fields("any"),
        },
        relative=True,
    )


class _LocatorQuery:
    def __init__(self, nodes: List[Dict[str, object]], root: int) -> None:
        self.nodes = nodes
        self.root = root

    def payload(self) -> Dict[str, object]:
        return {"nodes": self.nodes, "root": self.root}

    def current(self) -> Dict[str, object]:
        return self.nodes[self.root]


def _append_node(
    query: Optional[_LocatorQuery],
    node: Dict[str, object],
    *,
    relative: bool = False,
) -> _LocatorQuery:
    result = copy.deepcopy(query) if query is not None else _LocatorQuery([], 0)
    if len(result.nodes) >= 256:
        raise ValueError("locator expression exceeds 256 nodes")
    if relative:
        if query is None and node.get("direction", "within") != "within":
            raise ValueError("locator direction requires a parent locator")
        if query is not None:
            node["within"] = result.root
    result.root = len(result.nodes)
    result.nodes.append(node)
    return result


def _append_operand(query: _LocatorQuery, other: _LocatorQuery) -> int:
    offset = len(query.nodes)
    if offset + len(other.nodes) >= 256:
