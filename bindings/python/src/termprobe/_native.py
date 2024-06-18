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
    if isinstance(value, (abi.TuiOptionalU64, abi.TuiOptionalI32,
                          abi.TuiOptionalBool, abi.TuiOptionalF64)):
        return value.value if value.present else None
    if isinstance(value, abi.TuiColor):
        if value.kind == 0:
            return "default"
        if value.kind == 1:
            return value.index
        return "#{:02x}{:02x}{:02x}".format(value.red, value.green, value.blue)
    result = {}
    fields = {name for name, _ in value._fields_}
    for name, _ in value._fields_:
        if name.endswith("_len"):
            continue
        item = getattr(value, name)
        if name + "_len" in fields:
            item = [_copy(item[index]) for index in range(getattr(value, name + "_len"))]
        elif isinstance(item, C.Structure):
            item = _copy(item)
        result["char" if name == "character" else name] = item
    if isinstance(value, abi.TuiState):
        result["modes"] = {entry["name"]: entry["enabled"] for entry in result["modes"]}
    if isinstance(value, abi.TuiTerminalColors):
        result["palette"] = {int(entry["key"]): entry["value"] for entry in result["palette"]}
    return result


def _result(result, kind):
    if kind is None:
        return None
