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
