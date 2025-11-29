1. The year is 2025. Always use modern Rust module syntax, i.e. no `mod.rs`.

2. Group together imports using curly braces. Thus

```rust
use shared::{
    input::{UiKey, sanitize},
    player::UsernameError,
};
```

rather than

```rust
use shared::input::{UiKey, sanitize};
use shared::player::UsernameError;
```

3. By default, error messages should follow the Rust convention: all lowercase, no trailing punctuation, thus

```rust
format!("packet send failed: {}", e)
```

This is the rule for all error messages that might be propagated, for developer-facing error messages, and for messages delivered by the panic macro, assert, and except. The only exception is messages that we know are user-facing, such as the outermost error message that wraps all others. User-facing messages should follow normal English punctuation rules.

```rust
runner
    .ui
    .show_sanitized_error(&format!("No connection: {}.", e));
```

4. Comments should follow normal English punctuation, thus `// Like this.` rather than `// Like this`.
