# Netcode

## Local player: reconciliation, replay, and prediction

## Remote players: interpolation

##

# State Machines

## Client State Machine

```txt
Lobby -> Game -> AfterGameChat
```

Lobby has various substates, as detailed [below](#lobby).

From the Lobby substate `Connecting` onwards, any state can lead to `Disconnected`.

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
