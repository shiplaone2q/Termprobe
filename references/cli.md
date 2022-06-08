# CLI

Use the CLI for terminal work split across separate commands.

[Back to the skill](../SKILL.md)

## Start

| Command | Use |
| --- | --- |
| `open [options]` | Open a shell. |
| `run [options] PROGRAM [ARGS...]` | Run an app. |
| `[global options] -- PROGRAM [ARGS...]` | Alias for `run`. |
| `restart [--graceful-timeout MS]` | Restart the session. |
| `sessions` | List sessions. |
| `close [--all]` | Close sessions. |

Use `--session NAME` to select a session. `open` and `run` reuse it unless `--restart` is set.

`open` and `run` accept `--screen-history-limit COUNT` to control how many distinct recent screens are retained for failures.

`restart` replays the last successful spawn, preserving its original working directory, options, and latest terminal size. It sends Ctrl-C and waits up to 5000 ms before forcing replacement; `--graceful-timeout 0` skips the wait. It works after child exit, but not after `close` or daemon shutdown. The terminal and automatic recording start fresh.

## Locate text

```sh
termprobe find text "Save"
termprobe expect text "Save" --fg green
termprobe click text "Save" --after-text "Settings"
termprobe highlight text 'item \d+' --regex
```

Common options:

| Option | Use |
| --- | --- |
| `--regex` | Match a regular expression. |
| `--full` | Include scrollback. |
| `--whitespace exact\|normalize` | Match whitespace. |
| `--after-text TEXT` | Search after an anchor. |
| `--before-text TEXT` | Search before an anchor. |
| `--match any\|unique\|first\|last` | Select matches. |
| `--nth N` | Select a zero-based match. |

Style options: `--fg`, `--bg`, `--bold`, `--dim`, `--italic`, `--underline-style`, `--underline-color`, `--inverse`, `--hidden`, `--strikethrough`, and `--blink`.

`--link URL` matches that hyperlink on every cell, including spaces.
Use `--link ""` for unlinked cells. Other style filters ignore spaces.

`--link` is a separate whole-match constraint, not a style field. Native
Rust/JavaScript/Python locators also support cell-set AND/OR and locator-based
containment filters; the CLI does not expose a shell expression syntax for
those operations. Locator protocol requests carry expression queries rather
than native binding stage arrays.

`click text` also accepts `--button left|middle|right`, `--alt`, `--ctrl`, `--shift`, `--clicks`, and `--timeout`.

## Send input

| Command | Use |
| --- | --- |
| `submit [TEXT]` | Type and press Enter. |
| `type TEXT` | Type text. |
| `write DATA` | Write raw bytes. |
| `key press KEYS...` | Press keys. |
| `key down\|repeat\|up KEYS...` | Send key events. |
| `mouse click [X Y] [options]` | Click a cell or `--on-text`. |
| `mouse move X Y` | Move the pointer. |
| `mouse down\|up X Y [options]` | Press or release a button. |
| `mouse drag X1 Y1 X2 Y2 [options]` | Drag. |
| `mouse scroll up\|down [--amount N]` | Send mouse wheel input to the application. |
| `scroll up\|down [LINES]` | Move through native history; default: one row. |
| `scroll top\|bottom` | Show the oldest output or the live screen. |
| `resize COLS ROWS` | Resize. |
| `signal NAME` | Send a signal. |

Mouse button options are `--button left|middle|right`, `--alt`, `--ctrl`, and `--shift`.

## Native history

`get viewport --json` returns `{"value":{"offset":0,"history":N}}` within the response data.
The offset counts rows above the live screen. Zero means the live screen.
`history` counts the available history rows.
Scroll amounts must be integers from 0 through 2147483647.

`text`, locators, screenshots, and the monitor use the selected viewport.
`text --full` returns history plus the live screen, independent of that viewport.
New output preserves the selected historical view until retained history expires.
Live output follows automatically when history clears or a resize brings the viewport back to the live screen.
This also applies when resize changes the normal buffer while the alternate buffer is active.
Keyboard input returns to the live screen.
Mouse input and historical locator clicks fail while the viewport shows history.
Use `scroll bottom` before mouse input. Mouse behavior at the live screen does not change.
`get cursor` and `state` retain the application cursor coordinates.
Captures project that cursor into the viewport and hide it outside the view.
Raster recordings capture viewport changes. Asciicast records only the PTY stream.

See the runnable [JavaScript example](../bindings/js/examples/scrollback.mjs)
