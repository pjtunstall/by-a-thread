# Style guide

We're on the 2024 edition of Rust.

## Module file structure

Always use the modern module file structure, i.e. no `mod.rs`.

## Imports

### Group together nested items

Group together nested items using curly braces. Thus:

```rust
use common::{
    input::{UiKey, sanitize},
    player::UsernameError,
};
```

rather than

```rust
use common::input::{UiKey, sanitize};
use common::player::UsernameError;
```

### Group into three blocks: std, 3rd-party, and own

Group import statements at the head of a file into three paragraphs: std first, then 3rd party, then imports from my own project workspace.

```rust
use std::collections::HashMap;

use macroquad::prelude::*;
use renet::RenetClient;

use crate::{
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{self, player::Player};
```

## Error messages

By default, error messages should follow the Rust convention: all lowercase, no trailing punctuation.

```rust
format!("packet send failed: {}", e)
```

This is the rule for all error messages that might be propagated, for developer-facing error messages, and for messages delivered by the panic macro, assert, and except. The only exception is messages that we know are user-facing, such as the outermost error message that wraps all others. User-facing messages should follow normal English punctuation rules.

```rust
runner
    .ui
    .show_sanitized_error(&format!("No connection: {}.", e));
```

## Comments

Comments should follow normal English punctuation: `// Like this.` not `// Like this`.

## Naming

Favor explicit names: `buffer` not `buf`. You can make an exception for very common, conventional abbreviations and narrow contexts, e.g. `i` is fine for a loop index.

## Getters and setters

Don't introduce getter and setter methods unless they actually do something more than getting and setting, e.g. enforce an invariant. In that case, prefer expressive names to generic get (or calling the getter the same as the field) and set.
