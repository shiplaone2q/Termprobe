# Shared native library

`termprobe-ffi` exposes the Rust engine through one typed C ABI.
Go uses `purego`, Python uses `ctypes`, and JavaScript uses Koffi to load this library.
There is no separate Rust N-API adapter or JavaScript-specific native build.
Koffi supplies its own native addon.

The library and language package must come from the same source revision.
This ABI is an internal contract, not a stable public C SDK.
The language adapters preserve their public convenience APIs.
ABI version 4 includes the shared pointer and asynchronous entrypoints.
ABI version 5 adds native scrollback operations and the full-width `TuiViewport` result.
The existing `tui_screenshot` signature remains unchanged; `tui_screenshot_ptr` adds the pointer form.

## Build and use

From the repository root:

```sh
env -u CARGO_TARGET_DIR cargo build --locked -p termprobe-ffi
```

The output is `target/debug/libtermprobe_ffi.so` on Linux, `libtermprobe_ffi.dylib` on macOS, or `termprobe_ffi.dll` on Windows.

On musl Linux, disable static CRT linkage so Rust can produce the shared library:

```sh
RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=-crt-static" \
  env -u CARGO_TARGET_DIR cargo build --locked -p termprobe-ffi
```

Keep this flag for release builds and explicit musl targets.
For example, after you install `cargo-zigbuild` and the Rust target:

```sh
RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=-crt-static" \
  env -u CARGO_TARGET_DIR cargo zigbuild --locked -p termprobe-ffi --target x86_64-unknown-linux-musl
```

The cross-built library is `target/x86_64-unknown-linux-musl/debug/libtermprobe_ffi.so`.
It requires a musl process.

All three bindings accept `TERMPROBE__NATIVE_LIBRARY` as an explicit library path.
Python wheels and JavaScript platform packages include the library.

For example, run the Go smoke example on macOS:

```sh
export TERMPROBE__NATIVE_LIBRARY="$PWD/target/debug/libtermprobe_ffi.dylib"
cd bindings/go
