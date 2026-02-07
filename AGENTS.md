# Style Guide

## Use modern Rust

- Use the 2024 edition.
- Use modern module structure, i.e. no `mod.rs` files.

## Imports

- Group imports in curly braces, e.g. `use std::{collections::HashMap, io::Result};`.
- Group imports into paragraphs by category (standard library first, then external crates, then local modules) with an empty line between each paragraph.

## Errors

- In error messages, follow Rust convention and use lowercase initial letter and no trailing period.
- An exception, in this project, is that the outermost error message displayed to the user in the GUI should follow normal English convention, i.e. start with a capital letter and end with a period.

## Markdown

- Headings: first letter of first word uppercase, other words not automatically capitalized.

## Comments

- Don't introduce any new comments unless asked to.
- Only use `//` comments, not `///`.
- Comments should adhere to the conventions of standard English punctuation, inlcuding a trailing period.
- Comments should be addressed to readers who are not privy to our conversations or previous steps in development.

## Punctuation in comments and documentation

- Use ' for a single quote or apostrophe, and " for a double quote.
- Use -- for an m-dash.
