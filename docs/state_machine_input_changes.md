# State machine & input UI changes

Context: harden input visibility/polling and status messaging so the input box hides while waiting on the server, avoids dropped input, and prevents brief status flashes.

## Structural changes
- Added `InputMode` and `ClientSession::input_mode()` to derive whether input is `Enabled`, `DisabledWaiting`, or `Hidden` per state/flags (covers countdown/disconnect/game).
- Added waiting flags on session: `auth_waiting_for_server`, `chat_waiting_for_server`; `choice_sent` in difficulty already covered that flow.
- Added derived `input_ui_state()` on session with just `mode`. The previous status-line concept (with waiting debounce) has been removed.
- The main loop now:
  - Applies `input_ui_state` to `draw(show_input)`. Input polling runs only when `InputMode::Enabled`.

## Handler updates (signals only; no direct UI waiting calls)
- Connecting: marks auth waiting when sending the initial passcode.
- Authenticating: sets/clears `auth_waiting_for_server` around guesses; no direct “waiting” status calls.
- Chat: sets/clears `chat_waiting_for_server` around chat/start messages and server replies.
- Difficulty: uses `choice_sent` to enter waiting; resets `choice_sent/prompt_printed` on server info to re-enable input.
- Username: removes ad hoc “Waiting for server...” call when submitting username (now handled by derived state).
- Disconnection transitions now emit errors into the message history (in red) rather than a top-of-screen status line.

## Behavioral outcomes
- Input box hides whenever waiting flags (or choice_sent) are set; input polling stops in the same situations, eliminating dropped keystrokes during waits.
- Top-of-screen status line is gone; connectivity issues surface in the chat history with error styling.
- Countdown and future `InGame` states keep input hidden via `InputMode::Hidden`.
