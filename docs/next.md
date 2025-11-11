Fix this vulnerability:

```rust
fn is_participant_announcement(text: &str) -> bool {
    text.ends_with(" joined the chat.") || text.ends_with(" left the chat.")
}

fn is_roster_message(text: &str) -> bool {
    text.starts_with("Players online: ") || text == "You are the only player online."
}
```

Use `ServerMessage` in the lib.rs of shared.
