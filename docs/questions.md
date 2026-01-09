- Distribution of server game state data and server player.
  - Thin AoS: rigid body, consisting of position and velocity (presumably also orientation and angular velocity), then input buffers separately, then a HashMap from client ids to indices, then cold data (stuff like connection status that won't be used every tick).
  - Should I use pure SoA after all. It's not really any harder to think about, and it might encourage me into better habits. In any case, I can perform movement updates separately from collision detection.
  - The snapshot that's sent will only need position and orientation, not velocity. Should it be `Vec<Option<PlayerState>>` or just `Vec<PlayerState>`. Clients will receive notifications that players are out of the game via reliable channel, so they'll know already and won't have to be told each time who to ignore. But would the chance to send `None` for knocked-out players (instead of a default value) save on bandwidth?
    - Better than both: send a validity mask along with a `[Vec<f32>; 4]`.
  - Collision detection, with its branchy logic, tends to prevent SIMD optimization; hence keeping it separate from position updates allows the latter to be optimized.

I need to press on. I can always switch to SoA for players as a later refinement.
