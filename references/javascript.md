# JavaScript

Use `@microsoft/termprobe` from JavaScript or TypeScript.

[Back to the skill](../SKILL.md)

## Start

```sh
npm install @microsoft/termprobe
```

```js
import { Termprobe } from "@microsoft/termprobe";

const terminal = Termprobe.ephemeral();
try {
  await terminal.run("my-app");
  await terminal.getByText("Ready").expect();
  await terminal.getByText("Continue").click();
} finally {
  await terminal.closeQuiet();
}
```

## Locators

```js
const locator = terminal
  .getByText("Settings")
  .getByText("Save", { direction: "after" })
  .unique();
```

| Method | Use |
| --- | --- |
| `getByText(text, options?)` | Match text or regex. |
| `getByStyle(style, options?)` | Match appearance; when chained, require it on the whole match. |
| `getByLink(uri, options?)` | Match an OSC 8 target; when chained, require it on every cell. `""` means unlinked. |
| `and(other)`, `or(other)` | Intersect/union cells, then form contiguous per-row runs. |
| `filter({ has?, hasNot? })` | Keep whole matches containing an inner match or containing none. |
| `any()` | Keep all matches. |
| `unique()` | Require one match. |
| `first()`, `last()`, `nth(index)` | Select a match. |
| `locations()`, `location()`, `count()`, `all()` | Read matches. |
| `wait({ state?, timeout? })` | Wait for visible or hidden. |
| `expect({ not?, timeout? })` | Assert. |
| `click(options?)` | Click. |
| `highlight({ timeout? })` | Highlight. |

Text options: `regex`, `full`, `whitespace`, and chained `direction`.

Style/link options are `full` and chained `direction`. Filter accepts locators
only; use `getByText()` for text containment. A partially linked text match
passes `filter({ has: terminal.getByLink(uri) })` but fails chained
`getByLink(uri)`. `.and(terminal.getByLink(uri))` returns its linked cells.
Appearance refinement skips blanks when the match has visible characters;
link refinement checks blanks too.

Composition requires operands from the same terminal instance. It is lazy,
uses one snapshot, and merges adjacent selected cells even from different
matches. Separate rows remain separate runs; composed text uses exact-grid
whitespace. Apply `first()`/`nth()` after composition to select final runs.
Any `full` branch makes the entire query use the full grid.

Click options: `button`, `alt`, `ctrl`, `shift`, `clicks`, and `timeout`.
