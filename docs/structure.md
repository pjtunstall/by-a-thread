# State Machines

## Client State Machine

```txt
Lobby -> Game
```

After death, the client will enter a short out-of-body experience state (which might be its own state or handled as part of `Game`), then a debriefing state where they can chat and see a map of the maze with the locations of the other players, and a leaderboard showing the order they were killed. When only one is left, they'll be declared the winner and also join the chat. Or just show the leaderboard then an disconnect them.

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
