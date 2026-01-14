# Netcode

The client maintains an `input_history` ring buffer and a `snapshot_buffer` for player state updates from the server. The server maintains an `input_buffer` ring buffer for each player to store their inputs till it's time to process them.

## Local player: reconciliation, replay, and prediction

## Remote players: interpolation

## Bullets: extrapolation

# State Machines

## Client State Machine

```txt
Lobby -> Game -> AfterGameChat
```

Lobby has various substates, as detailed [below](#lobby).

From the Lobby substate `Connecting` onwards, any state (or substate) can lead to `Disconnected`.

### Lobby

```
Startup -> Connecting -> Authenticating -> ChoosingUsername <-> AwaitingUsernameConfirmation -> Chat
```

If the player is the host: `Chat -> ChoosingDifficulty`, otherwise `Chat -> Countdown`. In either case,

```txt
Countdown -> Game
```

## Server State Machine

```
Lobby -> ChoosingDifficulty -> Countdown -> Game
```

The `Game` state also manages clients in `AfterGameChat` since they arrive at different times.
