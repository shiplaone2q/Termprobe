# @microsoft/termprobe

Control, inspect, and test terminal apps from JavaScript or TypeScript.

## Install

```sh
npm install @microsoft/termprobe
```

Node 20+ is supported. The package is ESM only. Bun and Deno support is best effort. Deno 2 needs local `node_modules` and `--allow-ffi`.

Each platform package contains the `termprobe-ffi` shared library.
The JavaScript adapter uses Koffi to call the same typed C ABI as Go and Python.
It copies native results before it frees their Rust allocations.
The library stays loaded until process exit on all supported platforms.
Native operations can finish after JavaScript worker exit.
Native thread teardown releases unclaimed task handles and results without canceling operations or closing shared sessions.
Each binding copy uses its own anonymous Koffi layouts, so copies can share one Koffi module without type-name collisions.
Koffi supplies its own native addon; this package no longer builds a separate N-API adapter.
Keep optional dependencies enabled during installation; they contain the platform binaries for both packages.
The package needs no CLI.
Set `TERMPROBE__NATIVE_LIBRARY` to an absolute path to select a library from the same source revision.

For a local build, run `npm ci`, then `npm run build`.
Use `npm run build:native:debug` for a debug build.
These commands build `termprobe-ffi` and put the shared library in `native/`.
Run `cargo run --locked -p termprobe-bindgen` from the repository root to regenerate the native declarations.

For a distributable GNU Linux build, use:

```sh
npm run build:native -- --zig --target x86_64-unknown-linux-gnu
```

This requires Docker and builds the shared engine inside manylinux 2.28.
The container constrains both Cargo and Ghostty to the supported libc.
Use `aarch64-unknown-linux-gnu` for ARM64, with Docker emulation on an x86_64 host.
Musl builds with `--zig` still use the local Rust and Zig toolchains.

## Quick start

```js
import { Termprobe } from "@microsoft/termprobe";

const terminal = Termprobe.ephemeral();

try {
  await terminal.run(process.execPath, [
    "-e",
    "console.log('Ready'); setInterval(() => {}, 1000)",
  ]);
  await terminal.getByText("Ready").expect();
  console.log(await terminal.text());
} finally {
  await terminal.closeQuiet();
}
```

## API

### `Termprobe`

```ts
new Termprobe(session?: string, options?: ClientOptions)
```

| Option | Type | Default |
| --- | --- | --- |
| `session` | `string` | `TERMPROBE__SESSION` or `"default"` |
| `backend` | `"alacritty" \| "ghostty" \| "rio" \| "xtermjs"` | `"alacritty"` |
| `timeouts` | `Timeouts` | built-in defaults |
| `profile` | `Profile` | built-in profile |
| `screenHistoryLimit` | `number` | `10` |
| `artifacts` | `{ dir, onFailure?, includeRecording? }` | off |
| `recording` | `{ directory? }` | default recording directory |
| `trace` | `{ mode?, directory? }` | `{ mode: "off", directory: ".termprobe/traces" }` |

Set `trace.mode` to `"on"` for every session or `"on-failure"` for failures. Users can open `trace.html`; agents should read `trace.md`, `trace.json`, and `timeline.json`.

`artifacts.onFailure` selects `"none"`, `"text"`, `"html"`, or `"all"`. Use `includeRecording` to include the cast in failure artifacts.

#### Properties

| Property | Type |
| --- | --- |
| `session` | `string` |
| `keyboard` | `Keyboard` |
| `mouse` | `Mouse` |

#### Lifecycle

| Method | Description |
| --- | --- |
| `Termprobe.ephemeral(prefix?, options?)` | Create a unique session. |
| `open(options?)` | Open a shell. |
| `run(program, args?, options?)` | Run a program. |
| `restart(options?)` | Restart the session. |
| `close()` | Close the session. |
| `closeQuiet()` | Close without throwing. |
| `[Symbol.asyncDispose]()` | Close from `await using`. |

`open()` options are `shell`, `backend`, `cols`, `rows`, `cwd`, `env`, `waitReady`, `restart`, `retries`, `profile`, `timeouts`, and `screenHistoryLimit`. `run()` accepts the same options except `shell`.

The default size is 80 by 30. Timeout defaults are 5 seconds for text and idle, and 30 seconds for command, exit, and ready.

#### Input

| Method | Description |
