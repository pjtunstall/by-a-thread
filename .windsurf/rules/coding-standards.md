---
trigger: always_on
---
- Use modern Rust: 2024 edition.
  - In particular, use modern module structure, i.e. no `mod.rs` files.
- Group imports in curly braces, e.g. `use std::{collections::HashMap, io::Result};`.
- Group imports into paragraphs by category: standard library, external crates, local modules.
- Use normal English punctuation in comments and documentation, including trailing periods.
- In error messages, follow Rust convention and use lowercase initial letter and no trailing period.
  - An exception in this project is that the outermost error message displayed to the user in the GUI should follow normal English convention, i.e. start with a capital letter and end with a period.