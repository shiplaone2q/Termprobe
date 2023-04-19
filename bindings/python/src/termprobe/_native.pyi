# Private adapter types. The public API types live in tui_test.
# ruff: noqa: E501, F401, F403, F405

import builtins
import tui_test
import typing
__all__ = [
    "NativeAssertionError",
    "NativeInternalError",
    "NativeNoSessionError",
    "NativeSession",
    "NativeUsageError",
    "close_all",
    "recording",
    "sessions",
]

class NativeAssertionError(builtins.Exception):
    r"""
    Native assertion failure.
    """
    ...

class NativeInternalError(builtins.Exception):
    r"""
    Native internal error.
    """
    ...

class NativeNoSessionError(builtins.Exception):
    r"""
    Native session was not found.
    """
    ...

@typing.final
class NativeSession:
    @property
    def name(self) -> builtins.str: ...
    def __new__(cls, name: str, recording_directory: typing.Optional[str] = None, artifact_directory: typing.Optional[str] = None, artifact_mode: typing.Optional[str] = None, artifact_include_recording: bool = False, trace_mode: typing.Optional[str] = None, trace_directory: typing.Optional[str] = None) -> NativeSession: ...
