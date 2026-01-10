Check consistency of decode/unexpected error handling on client. Server is ok. But consider case by case as some may suit business logic.

- state.rs logs unexpected variants via variant_name() and logs decode errors to stderr, then continues.
- chat.rs, auth.rs, waiting.rs ignore unexpected variants and surface decode errors to the UI with bracketed messages.
- connecting.rs ignores both unexpected variants and decode errors completely.
- username.rs ignores decode errors (only handles Ok); unexpected variants are silently ignored via handle_server_message.
- countdown.rs ignores all decoded messages and shows a UI error on decode failures (with different capitalization/punctuation).
- difficulty.rs ignores unexpected variants and shows a UI error on decode failure, but the message format is different ([DESERIALIZATION ERROR: ...]).

Look at the variety of period, versus semicolon, versus colonin logs.
