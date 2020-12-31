# Rust

Use `termprobe-rs` from Rust code and tests.

[Back to the skill](../SKILL.md)

```sh
cargo add termprobe-rs@0.1.1
```

```rust
use termprobe::{OpenOptions, Session};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terminal = Session::new("example");
