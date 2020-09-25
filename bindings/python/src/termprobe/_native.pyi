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
