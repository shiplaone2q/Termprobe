---
name: termprobe
description: "Control, inspect, test, and record real terminal apps with the termprobe CLI or its Rust, Python, and JavaScript APIs. Use for shells, TUI apps, text and style locators, keyboard or mouse input, terminal state, clipboard waits, screenshots, recordings, snapshots, and live terminal sessions."
---

# termprobe

Use `termprobe` to control and test a real terminal.

## Pick an API

| Context | Use | Reference |
| --- | --- | --- |
| Agent or shell workflow | CLI | [CLI](references/cli.md) |
| Python code | `termprobe` | [Python](references/python.md) |
| JavaScript or TypeScript | `@microsoft/termprobe` | [JavaScript](references/javascript.md) |
| Rust code | `termprobe-rs` | [Rust](references/rust.md) |
| Common tasks | Current project language | [Recipes](references/recipes.md) |

Use the library that matches the project. Use the CLI when the terminal must persist across separate commands or be watched with `monitor`.

CLI and library sessions do not share state.

## CLI

```sh
termprobe run my-app
termprobe expect text "Ready"
termprobe click text "Continue"
termprobe expect text "Done"
termprobe close
```

Run `termprobe agent-context` for exact command and option names.

## Python

```python
from termprobe import Termprobe

async with Termprobe.ephemeral() as terminal:
    await terminal.run("my-app")
    await terminal.get_by_text("Ready").expect()
    await terminal.get_by_text("Continue").click()
```
