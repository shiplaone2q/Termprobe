# termprobe for Python

Control, inspect, and test terminal apps from Python.

## Install

```sh
pip install termprobe
```

Python 3.8+ is supported.

The wheel contains an ordinary shared library. Python uses generated `ctypes`
declarations for its typed C ABI. The package has no Python native extension.

Set `TERMPROBE__NATIVE_LIBRARY` to an absolute library path to override the bundled
library. The loader checks the ABI version before the first operation.

### Build from source

Install Rust 1.90 or later and the build dependencies for the supported terminal
backends. Build a wheel with the standard Python build frontend:

```sh
python -m pip install build
env -u CARGO_TARGET_DIR python -m build bindings/python
```

The build uses `setuptools-rust` with `Binding.NoBinding`. Wheels use
`py3-none-<platform>` tags. The source distribution includes the shared ABI,
the Rust engine, their assets, and the Cargo configuration that separates backend symbols.
Source builds need Cargo dependency access.

For cross builds, set `CARGO_BUILD_TARGET` and pass
`-C--build-option=--plat-name=<platform>` to `python -m build --wheel`.
Build Linux release wheels in the required manylinux or musllinux container.
Use `auditwheel repair` before publication. Setuptools-rust disables static
libc linkage for musl shared libraries.

For a checkout, use the shared library directly:

```sh
env -u CARGO_TARGET_DIR cargo build -p termprobe-ffi
PYTHONPATH=bindings/python/src \
TERMPROBE__NATIVE_LIBRARY="$PWD/target/debug/libtermprobe_ffi.dylib" \
python bindings/python/examples/shared_library.py
```

Use `libtermprobe_ffi.so` on Linux or `termprobe_ffi.dll` on Windows.
The example proves that a copied screen remains valid after the session closes.

Each asynchronous call uses a separate worker thread. A blocked wait cannot
prevent `close()` from interrupting waits. Cancellation releases each native
result when its call returns.

Cancellation stops the Python await, not the native operation. A canceled
`open()`, `run()`, or `restart()` can still complete. The adapter does not close
the session automatically after cancellation. This preserves any replacement
session with the same name. The caller remains responsible for explicit session
cleanup; cancellation does not confirm that a spawn stopped.

## Quick start

```python
from termprobe import Termprobe

async with Termprobe.ephemeral() as terminal:
    await terminal.run("my-app")
    await terminal.get_by_text("Ready").expect()
    await terminal.get_by_text("Continue").click()
    await terminal.get_by_text("Done").expect()
```

## API

### `Termprobe`

```python
Termprobe(session=None, *, backend=None, timeouts=None, profile=None, screen_history_limit=None, artifacts=None, recording=None)
```

| Option | Type | Default |
| --- | --- | --- |
| `session` | `str` | `TERMPROBE__SESSION` or `"default"` |
| `backend` | `"alacritty" \| "ghostty" \| "rio" \| "xtermjs"` | `"alacritty"` |
| `timeouts` | `Timeouts \| dict` | built-in defaults |
| `profile` | `Profile \| dict` | built-in profile |
| `screen_history_limit` | `int \| None` | core default |
| `artifacts` | `dict` | off |
| `recording` | `AutomaticRecording \| dict` (directory only) | default recording directory |
| `trace` | `TraceOptions \| dict` | `{"mode": "off", "directory": ".termprobe/traces"}` |

Set `trace.mode` to `"on"` for every session or `"on-failure"` for failures. Users can open `trace.html`; agents should read `trace.md`, `trace.json`, and `timeline.json`.

`artifacts["on_failure"]` selects `"none"`, `"text"`, `"html"`, or `"all"`. Use `include_recording` to include the cast in failure artifacts.

#### Properties

| Property | Type |
| --- | --- |
| `session` | `str` |
| `keyboard` | keyboard helper |
| `mouse` | mouse helper |

#### Lifecycle

| Method | Description |
| --- | --- |
| `Termprobe.ephemeral(prefix=None, **options)` | Create a unique session. |
| `await open(**options)` | Open a shell. |
| `await run(program, *args, **options)` | Run a program. |
| `await restart(graceful_timeout=5000)` | Restart the session. |
| `await close()` | Close the session. |
| `await close_quiet()` | Close without raising. |
| `async with Termprobe()` | Close on exit. |

