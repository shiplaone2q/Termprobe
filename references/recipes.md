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
