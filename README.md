# termprobe

`termprobe` controls, inspects, tests, and records real shell sessions and full-screen terminal apps on Windows, Linux, and macOS. Use it from the CLI or call the same engine from Rust, Python, JavaScript, or Go. It works for AI agents that need structured access to terminal state, terminal automation, and terminal ui application testing.

<p align="center">
  <a href="#installation">Installation</a>
  ·
  <a href="#quick-start">Quick start</a>
  ·
  <a href="#agent-commands">AI agents</a>
  ·
  <a href="#api-references">API references</a>
  ·
  <a href="#configuration">Configuration</a>
</p>

## Installation

### CLI

#### Homebrew

```sh
brew tap microsoft/termprobe https://github.com/microsoft/termprobe
brew install termprobe
```

#### Install script

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/microsoft/termprobe/main/install/install.sh | sh
```

Windows:

```powershell
irm https://raw.githubusercontent.com/microsoft/termprobe/main/install/install.ps1 | iex
```

You can also download a binary from GitHub Releases.

### Libraries

| Language | Install | Reference |
| --- | --- | --- |
| Rust 1.90+ | `cargo add termprobe-rs@0.1.1` | [docs.rs](https://docs.rs/termprobe-rs/latest/termprobe/) |
| Python 3.8+ | `pip install termprobe` | [Python API](bindings/python/README.md) |
| Node 20+ | `npm install @microsoft/termprobe` | [JavaScript API](bindings/js/README.md) |
| Go 1.26+ | `go get github.com/microsoft/termprobe/bindings/go@v0.1.1` | [Go API](bindings/go/README.md) |

Add the Rust `recording-raster` feature for APNG, GIF, and MP4 output. It uses installed fonts; `recording-font-jetbrains-mono*` bundles a font.

Go, Python, and JavaScript use one [native adapter crate](crates/termprobe-ffi/README.md).
All three use its generated C ABI declarations through purego, ctypes, and Koffi, respectively.
Set `TERMPROBE__NATIVE_LIBRARY` to select a matching library explicitly.

## Quick start

The CLI and libraries expose the same terminal actions. Python, JavaScript, Rust, and Go sessions run in-process and do not require the CLI. Released Go modules include the native engine; see [Go installation](bindings/go/README.md#install) for supported platforms and Linux runtime requirements.

### CLI

```sh
termprobe run my-app
termprobe expect text "Ready"
termprobe click text "Continue"
termprobe expect text "Done"
termprobe screenshot -o result.svg
termprobe close
```

### Python

```python
import asyncio
from termprobe import Termprobe

async def main():
    async with Termprobe.ephemeral() as terminal:
        await terminal.run("my-app")
        await terminal.get_by_text("Ready").expect()
        await terminal.get_by_text("Continue").click()
        await terminal.get_by_text("Done").expect()

asyncio.run(main())
```

[Python API reference](bindings/python/README.md)

### JavaScript

```js
import { Termprobe } from "@microsoft/termprobe";

const terminal = Termprobe.ephemeral();

try {
  await terminal.run("my-app");
  await terminal.getByText("Ready").expect();
  await terminal.getByText("Continue").click();
  await terminal.getByText("Done").expect();
} finally {
  await terminal.closeQuiet();
}
```

[JavaScript API reference](bindings/js/README.md)

### Rust

```rust
use termprobe::{OpenOptions, Operation, Session};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terminal = Session::new("example");
    terminal.open(OpenOptions::default())?;
    terminal.execute(Operation::Submit {
        data: Some("echo hello".into()),
    })?;
    terminal.get_by_text("hello").last().expect()?;
    terminal.close()?;
    Ok(())
}
```

[Rust API reference](https://docs.rs/termprobe-rs/latest/termprobe/)

### Go

```go
package main

import (
    "log"

    "github.com/microsoft/termprobe/bindings/go"
)

func main() {
    terminal, err := termprobe.Ephemeral("example", termprobe.ClientOptions{})
    if err != nil {
        log.Fatal(err)
    }
    defer terminal.CloseQuiet()

    if _, err := terminal.Run("my-app", nil, termprobe.SpawnOptions{}); err != nil {
        log.Fatal(err)
    }
    if err := terminal.GetByText("Ready", termprobe.TextSelectorOptions{}).Expect(termprobe.LocatorExpectOptions{}); err != nil {
        log.Fatal(err)
    }
}
```
