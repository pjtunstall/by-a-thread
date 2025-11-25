# Client architecture notes

Observations on how the current client pieces fit together and options for moving toward a clearer three-phase split (Lobby, Game, Debrief) for the Macroquad-based multiplayer FPS.

## Current roles

- **ClientRunner** (`client/src/run.rs`): Owns the live objects (Renet client/transport, `ClientSession`, `LobbyUi`, assets) and drives the async frame loop. It pumps the network, forwards to state handlers, draws UI each frame, and handles the special countdown-to-game transition. It also reacts to user escape and forces disconnect transitions. UI side-effects during transitions are triggered here via `apply_client_transition`.
- **ClientSession** (`client/src/session.rs`): Holds identity (`client_id`, `is_host`), wall-clock estimate, and the current `ClientState`. It keeps an input queue for chat/prompt entry, controls UI gating (`InputMode`), tracks delayed “waiting…” messaging, and owns helpers for specific states (username prompt printed, waiting flags, roster expectation). It includes the zero-copy `transition_to_game` bridge from countdown to in-game data.
- **ClientState** (`client/src/state.rs`): Enum capturing all transient UI/network states: startup/auth/username, chat lobby, difficulty selection, countdown, in-game, and disconnection variants. Also defines `InputMode` and the `Game` data snapshot (maze + players) with a simple draw helper.
- **ClientUi** and **MacroquadUi** (`client/src/ui.rs`, `client/src/ui/mq.rs`): `ClientUi` is the UI facade (text output, prompts, key polling, countdown drawing). `MacroquadUi` implements it with a console-like on-screen overlay, input buffer management, and text wrapping. UI is responsible for sanitizing display strings as requested by callers.
- **State handlers** (`client/src/state_handlers/*.rs`): Pure-ish functions that implement per-state logic, pulling messages from the network, consuming session input, emitting UI output, and returning optional next states. Runner dispatches to them based on `ClientState`.

## Friction in the current layout

- Responsibilities blur between `ClientRunner` (transition orchestration), `ClientSession` (state data + UI gating + timers), and the handlers (logic + side-effects). Side-effects currently sit in both `apply_client_transition` (e.g., printing prompts) and the handlers, so mental model of “who mutates UI/session and when” is spread out.
- `ClientState` mixes connection micro-states (startup/authentication/username) with long-lived phases (`Chat`, `Game`). This increases the state surface and makes it harder to reason about high-level phase transitions.
- Input is coupled to the chat-style lobby; in-game currently bypasses the shared pipeline (custom camera setup in `run.rs`). The `InputMode` gating lives inside `ClientSession`, meaning UI rules are partly in state data.
- Transition actions have two pathways (generic ChangeTo vs. StartGame swap) that are special-cased in `run.rs`, not in a shared transition manager.

## Targeting Lobby → Game → Debrief

Think in terms of three coarse phases, each owning its own set of substates and presentation. Options:

1. **Hierarchical state machine (phases with sub-states)**

   - Top-level enum `Phase { Lobby(LobbyState), Game(GameState), Debrief(DebriefState) }`.
   - `LobbyState` would absorb current startup/auth/username/chat/difficulty/countdown substates. `GameState` covers active play; `DebriefState` can manage scoreboards/summary/return-to-lobby decisions.
   - A single dispatcher can call `Phase::update(...)` which delegates to the sub-handler for the active phase. Transitions stay inside the phase module (returning `PhaseTransition` describing next phase + payload).

2. **Trait-driven state objects**

   - Define `trait ClientPhase { fn enter(&mut ...); fn update(&mut ...); fn exit(&mut ...); }` implemented by `LobbyPhase`, `GamePhase`, `DebriefPhase`.
   - Runner holds a boxed `dyn ClientPhase`, swapping on transition messages. Helpful if phases need owned resources or background tasks that live across frames.

3. **Scene stack / controller with data pods**
   - Keep `ClientSession` for stable identity/timekeeping and split data into pods: `LobbyData`, `GameData`, `DebriefData`.
   - A scene controller owns the active pod and injects shared services (network, ui). This keeps data layout explicit and makes serialization/replay possible later.

## Suggested tidy-ups before or during the split

- **Clarify responsibilities**: Move UI-side-effects for transitions into the phase/handler modules so `ClientRunner` only applies the transition outcome. Let `ClientSession` hold state data and timing, but push UI gating (`InputMode`) decisions into the active phase handler or UI adapter.
- **Normalize transitions**: Replace `TransitionAction` with a richer `PhaseTransition` struct (target phase, optional payload, disconnect flag, reason). Centralize the countdown-to-game handoff there instead of a bespoke `StartGame` path.
- **Lobby module**: Encapsulate auth/username/chat/difficulty/countdown as an internal mini-state machine under a `LobbyPhase`, reducing the surface of `ClientState` exposed to the rest of the client. This is also the right place to manage host-only actions (start match) and roster visibility.
- **Game module**: Separate render/update loop pieces so input handling, camera control, and networking updates are driven through the same phase interface (instead of `run.rs` special-casing it). Prepare data structures for player prediction/interp and map assets to support FPS controls.
- **Debrief module**: Design for match results, chat/rematch voting, and transition back to lobby. It likely needs read-only access to the final `GameData` snapshot and minimal network messaging.
- **Shared services**: Consider a small `ClientContext` passed to phases containing `NetworkHandle`, `Resources`, `Time`, and `Ui` references. This simplifies signatures and makes future testing/mocking easier.

## Possible next steps

- Sketch the new phase enum/trait and map existing states into Lobby sub-states. Decide whether `ClientSession` remains or is split into `CoreSession` (id/time/host flags) plus per-phase data pods.
- Pull transition UI effects into the handlers/phases and make `apply_client_transition` a pure state swapper.
- Define the Debrief requirements (what data to show, how to exit) so Lobby ↔ Game ↔ Debrief transitions can be described in terms of explicit payloads.
- Align input handling: unify chat/prompt input and in-game controls under the phase system so Macroquad UI only renders what the phase asks for.
