```rust
let head = self.snapshot_buffer.head;
if matches!(self.last_reconciled_tick, Some(last) if head <= last) {
    return;
}
```

Equivalent to

```rust
let head = self.snapshot_buffer.head;

if let Some(last) = self.last_reconciled_tick {
    if head <= last {
        return;
    }
}
```
