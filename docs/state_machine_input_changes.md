# State machine & input UI changes

Context: harden input visibility/polling and status messaging so the input box hides while waiting on the server, avoids dropped input, and prevents brief status flashes.

## Structural changes
- Added `InputMode` and `ClientSession::input_mode()` to derive whether input is `Enabled`, `DisabledWaiting`, or `Hidden` per state/flags (covers countdown/disconnect/game).
- Added waiting flags on session: `auth_waiting_for_server`, `chat_waiting_for_server`; `choice_sent` in difficulty already covered that flow.
- Added derived `input_ui_state()` on session with `mode` + `status_line`. Status defaults to “Waiting for server…” when in `DisabledWaiting`, but only after a 300ms debounce (tracked via `waiting_since`). Explicit status messages use `session.set_status_line(...)`.
- The main loop now:
  - Updates the waiting timer each frame based on `input_mode`.
  - Applies `input_ui_state` to both `show_status_line` and `draw(show_input)`. Input polling runs only when `InputMode::Enabled`.

## Handler updates (signals only; no direct UI waiting calls)
- Connecting: marks auth waiting when sending the initial passcode and clears any prior status line.
- Authenticating: sets/clears `auth_waiting_for_server` around guesses; clears status on server responses; no direct “waiting” status calls.
- Chat: sets/clears `chat_waiting_for_server` around chat/start messages and server replies.
- Difficulty: uses `choice_sent` to enter waiting; resets `choice_sent/prompt_printed` on server info to re-enable input.
- Username: removes ad hoc “Waiting for server...” call when submitting username (now handled by derived state).
- Disconnection transitions set status via session, not UI directly.

## Behavioral outcomes
- Input box hides whenever waiting flags (or choice_sent) are set; input polling stops in the same situations, eliminating dropped keystrokes during waits.
- “Waiting for server...” only appears if waiting lasts >300ms; brief, fast replies won’t flash the message.
- Countdown and future `InGame` states keep input hidden via `InputMode::Hidden`.
