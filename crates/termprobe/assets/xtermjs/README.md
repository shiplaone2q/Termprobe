# Vendored xterm.js assets

Compiled into the `termprobe` binary by `crates/termprobe/src/terminal/xtermjs.rs`
so the xterm.js backend needs no Node.js at runtime.

| File | Source | License |
| ---- | ------ | ------- |
| `xterm-headless.js` | [`@xterm/headless`](https://www.npmjs.com/package/@xterm/headless) | MIT |
| `addon-unicode11.js` | [`@xterm/addon-unicode11`](https://www.npmjs.com/package/@xterm/addon-unicode11) | MIT |
| `LICENSE` | both of the above, reproduced per package | MIT |

`shim.js` is termprobe's own code, not vendored.

Both bundles are dropped in unchanged and committed, so building termprobe needs
neither Node nor a network. `pinned.json` records the versions they came from.

## Why the unicode11 addon

The headless bundle ships only the Unicode 6 width tables, which measure astral
emoji as one column. alacritty measures them as two, so without this a line
containing an emoji reports every following cell in a different column on the
