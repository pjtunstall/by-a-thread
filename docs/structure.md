# State Machines

## Client State Machine

```txt
Lobby -> Game
```

Lobby has various substates, as detailed [below](#lobby).

From the Lobby substate `Connecting` onwards, any state can lead to `Disconnected`.

Yet to be implemented: after death, the client will enter a short out-of-body experience state (which might be its own state or handled as part of `Game`), then a debriefing state where they can chat and see a map of the maze with the locations of the other players, and a leaderboard showing the order they were killed. When only one is left, they'll be declared the winner and also join the chat. Or just show the leaderboard then an disconnect them.

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
