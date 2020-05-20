"""Private Python adapter for the shared, typed C ABI."""

import asyncio
import ctypes as C
import json
import os
from pathlib import Path
import sys
import threading

from . import _abi as abi


class NativeAssertionError(Exception):
    pass


class NativeUsageError(Exception):
    pass


class NativeNoSessionError(Exception):
    pass


class NativeInternalError(Exception):
    pass


_ERRORS = {
    1: ("assertion", NativeAssertionError),
    2: ("usage", NativeUsageError),
    3: ("no_session", NativeNoSessionError),
    5: ("internal", NativeInternalError),
}
_library = None
_library_lock = threading.Lock()


def _error(code, message, details=None, artifact=None):
    kind, cls = _ERRORS.get(code, _ERRORS[5])
    error = cls(message)
    error._tui_test_error_json = json.dumps(
        dict(kind=kind, message=message, details=details, artifact=artifact)
    )
    return error


def _load():
    global _library
    with _library_lock:
        if _library is None:
            filename = (
                "tui_test_ffi.dll" if sys.platform == "win32"
                else "libtui_test_ffi.dylib" if sys.platform == "darwin"
                else "libtui_test_ffi.so"
            )
            path = os.environ.get("TUI_TEST_NATIVE_LIBRARY")
            if not path:
                path = str(Path(__file__).with_name(filename))
            try:
                library = C.CDLL(path)
                abi.configure(library)
            except (OSError, AttributeError) as error:
                raise _error(5, "cannot load native library {!r}: {}".format(path, error)) from error
            version = library.tui_abi_version()
            if version != abi.ABI_VERSION:
                raise _error(5, "native ABI version mismatch: expected {}, got {}".format(
                    abi.ABI_VERSION, version
                ))
            _library = library
        return _library


def _integer(value, name, bits=64, signed=False):
    if isinstance(value, bool):
        raise _error(2, "{} must be an integer".format(name))
    try:
        value = value.__index__()
    except Exception:
        raise _error(2, "{} must be an integer".format(name)) from None
    if isinstance(value, bool) or not isinstance(value, int):
        raise _error(2, "{} must be an integer".format(name))
    minimum = -(1 << (bits - 1)) if signed else 0
    maximum = (1 << (bits - int(signed))) - 1
    if not minimum <= value <= maximum:
        raise _error(2, "{} must be an integer between {} and {}".format(name, minimum, maximum))
    return value


def _optional(value, name="timeout_ms", bits=64):
    return abi.TuiOptionalU64(
        value is not None, 0 if value is None else _integer(value, name, bits)
    )


def _boolean(value):
    return abi.TuiOptionalBool(value is not None, False if value is None else value)


def _float(value):
    return abi.TuiOptionalF64(value is not None, 0.0 if value is None else value)


def _string(value):
    if value is None:
        return abi.TuiString()
    if not isinstance(value, str):
        raise TypeError("expected a string")
    encoded = value.encode("utf-8")
    # The extra byte keeps an explicit empty string distinct from NULL.
    buffer = C.create_string_buffer(encoded)
    return abi.TuiString(C.cast(buffer, C.POINTER(C.c_uint8)), len(encoded))


def _strings(values):
    return (abi.TuiString * len(values))(*(_string(value) for value in values))


def _pairs(values):
    return (abi.TuiPair * len(values))(*(
        abi.TuiPair(_string(key), _string(value)) for key, value in values
    ))


def _text(value):
    if not value.data:
        return None
    return C.string_at(value.data, value.len).decode("utf-8")


def _copy(value):
    """Copy every borrowed field before the result allocation is released."""
    if isinstance(value, abi.TuiString):
        return _text(value)
