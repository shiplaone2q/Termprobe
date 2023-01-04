# Python

Use `termprobe` from Python code and tests.

[Back to the skill](../SKILL.md)

## Start

```sh
pip install termprobe
```

```python
from termprobe import Termprobe

async with Termprobe.ephemeral() as terminal:
    await terminal.run("my-app")
    await terminal.get_by_text("Ready").expect()
    await terminal.get_by_text("Continue").click()
```

## Locators

```python
locator = (
    terminal
    .get_by_text("Settings")
    .get_by_text("Save", direction="after")
    .unique()
)
```

| Method | Use |
