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
| --- | --- |
| `get_by_text(text, **options)` | Match text or regex. |
| `get_by_style(style, **options)` | Match appearance; when chained, require it on the whole match. |
| `get_by_link(uri, **options)` | Match an OSC 8 target; when chained, require it on every cell. `""` means unlinked. |
| `and_(other)`, `or_(other)` | Intersect/union cells, then form contiguous per-row runs. |
| `filter(has=..., has_not=...)` | Keep whole matches containing an inner match or containing none. |
| `any()` | Keep all matches. |
| `unique()` | Require one match. |
| `first()`, `last()`, `nth(index)` | Select a match. |
| `locations()`, `location()`, `count()`, `all()` | Read matches. |
