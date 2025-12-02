# Style guide

We're on the 2024 edition of Rust.

## Module file structure

Always use the modern module file structure, i.e. no `mod.rs`.

## Imports

### Group into three blocks: std, 3rd-party, and own

Group import statements at the head of a file into three blocks: std first, then 3rd party, then imports from my own project workspace.

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

### Sort into three sections

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

Comments should follow normal English punctuation: `// Like this.` rather than `// Like this`.
