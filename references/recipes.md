# Recipes

[Back to the skill](../SKILL.md)

## Run a command

```sh
termprobe open
termprobe submit "npm test"
termprobe wait command
termprobe expect exit-code 0
```

## Drive an app

```sh
termprobe run my-app
termprobe expect text "Ready"
termprobe click text "Continue"
termprobe expect text "Done"
```

## Match relative text

```sh
termprobe click text "Save" --after-text "Settings" --match unique
```

```python
await (
    terminal
    .get_by_text("Settings")
    .get_by_text("Save", direction="after")
    .unique()
    .click()
)
```

```js
await terminal
  .getByText("Settings")
  .getByText("Save", { direction: "after" })
  .unique()
  .click();
```

## Wait for loading

```python
loading = terminal.get_by_text("Loading")
await loading.wait()
await loading.wait(state="hidden")
await terminal.get_by_text("Ready").expect()
```

## Click with modifiers

```sh
termprobe click text "Open" --button right --ctrl
```

```python
await terminal.get_by_text("Open").click(button="right", ctrl=True)
```

```js
await terminal.getByText("Open").click({ button: "right", ctrl: true });
```

## Wait for clipboard

```python
await terminal.wait_clipboard("copied")
```

```js
await terminal.waitClipboard(/copied/i);
```
