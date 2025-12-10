# Netcode Plan

## Rates and Channels

- Simulation: 60 Hz on both client and server (tick = 16.67 ms).
- Broadcast: 20 Hz snapshots on `Unreliable` (positions, orientations, health).
- Critical events (player death, bullet spawn/bounce/expiry) on `ReliableOrdered`.
- Clients send redundant inputs (4 recent inputs per packet) on `Unreliable`.

## Buffers and IDs

- Use power-of-two ring buffers so tick wrap fits in `u16`.
  - Server input buffer per player: 128 entries (~2.1 s).
  - Client input history: 256 entries (~4.3 s).
  - Snapshot buffer for interpolation: 16 entries (~0.8 s at 20 Hz).
- Inputs carry the tick the client wants processed (server_tick_target). Server should verify tick matches slot before using; clear processed slots to default value to avoid stale reuse.
- Safety caps: if no input from a client for 0.5 s (~30 ticks), treat as no input.

## Server Loop

- Each tick: for every player, pop input at `tick & (INPUT_BUFFER_LEN - 1)` if the tick matches; otherwise reuse the last valid input (or None after safety cap).
- Advance physics at fixed 60 Hz timestep.
- At broadcast cadence: send snapshot (tick-tagged) with states of players unreliably; send critical events reliably.
- Store player state in a `Vec<ServerPlayer>` within game state. (Or consider an array.)

## Client Loop (Local Player)

- Maintain an [estimated server clock](../client/src/time.rs): smooth RTT, clamp spikes, snap when drift > ~250 ms, otherwise apply capped proportional corrections.
- Compute target server tick: `estimated_server_time + smoothed_rtt/2 + jitter_margin` (`~50 ms` configurable) divided by tick duration.
- Record current input in input history at `tick & (INPUT_HISTORY_LEN - 1)` and send the latest few inputs redundantly.
- When a new snapshot arrives, set it as baseline, reconcile to it, then replay queued inputs (prediction) up to current tick before rendering.
- Fixed timestep for simulation; cap catch-up work per frame to avoid spirals; discard leftover accumulator if limit is hit.
- Process reliable messages before unreliable so death state halts reconciliation/prediction.

## Remote Players

- Keep a snapshot ring buffer; render at `estimated_server_time - INTERPOLATION_DELAY` (e.g., 100 ms). Interpolate between the two surrounding snapshots; if missing a later snapshot, assume no movement.
- I.e. prefer interpolation over extrapolation to mitigate jitter and low broadcast rate.

## Bullets

- Do not put bullets in the snapshot buffer. Maintain a live `Vec<Bullet>`; update via reliable events (spawn, bounce, expiry, kills). Extrapolate own bullets to “now” using supplied position/velocity(direction)/tick. Extrapolate bullets fired by remote players to their render time, ~100ms in the local player's past. When the local player fires, immediately create a provisional bullet; replace it with the server's version when that arrives. Verify or vanish: destroy the prospective bullet (disappear or fade out) if no confirmation arrives after 0.5s.

## Serialization Notes

- Separate wire format from in-memory layout; consider masks to send only active players.
- Ensure snapshot and buffer sizes stay powers of two; assert at init.

## Open Questions / Caveats

- Jitter margin currently ~50 ms; may need to scale with observed RTT if late inputs occur.
- Consider delta compression: have the server only send what changed.
