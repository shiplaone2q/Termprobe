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
