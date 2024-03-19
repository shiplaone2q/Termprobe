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
    def open(self, shell: typing.Optional[str], backend: typing.Optional[str], cols: int, rows: int, cwd: typing.Optional[str], env: typing.List[typing.Tuple[str, str]], wait_ready: typing.Optional[bool], restart: bool, profile_scrollback: typing.Optional[int], profile_colors: typing.List[typing.Tuple[str, str]], text_timeout: typing.Optional[int], idle_timeout: typing.Optional[int], command_timeout: typing.Optional[int], exit_timeout: typing.Optional[int], ready_timeout: typing.Optional[int], screen_history_limit: typing.Optional[int]) -> typing.Awaitable[tui_test.OpenResult]: ...
    def run(self, program: str, args: typing.List[str], backend: typing.Optional[str], cols: int, rows: int, cwd: typing.Optional[str], env: typing.List[typing.Tuple[str, str]], wait_ready: typing.Optional[bool], restart: bool, profile_scrollback: typing.Optional[int], profile_colors: typing.List[typing.Tuple[str, str]], text_timeout: typing.Optional[int], idle_timeout: typing.Optional[int], command_timeout: typing.Optional[int], exit_timeout: typing.Optional[int], ready_timeout: typing.Optional[int], screen_history_limit: typing.Optional[int]) -> typing.Awaitable[tui_test.OpenResult]: ...
    def restart(self, graceful_timeout_ms: int) -> typing.Awaitable[tui_test.OpenResult]: ...
    def close(self, failed: typing.Optional[bool] = None) -> typing.Awaitable[None]: ...
    def state(self) -> typing.Awaitable[typing.Dict[str, typing.Any]]: ...
