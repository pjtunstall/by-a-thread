# State Machines

## Client State Machine

```txt
Lobby -> Game
```

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
