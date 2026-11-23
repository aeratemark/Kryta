# 0018 - The walk to an agent points the client along the route

**Date:** 2026-11-23. **Decided by:** the owner.

## The decision

When a player walks to an agent (spacebar or a click on an NPC, `0x8039`; a party member, `0x8033`),
`game/Movement/Approach.rs` `steer` now:

- densifies the finder's route with `densify(..., LIVE_CADENCE_STEP)`, as a click's walk does
  (`game/Handlers/MovementHandler.rs`, `answer_move_to_coord`);
- after the `0x002A` it already sends, sends `0x0029` (`agent_move_to_point`) for the route's first
  point when the walk sets off, and again at the announce cadence (`ANNOUNCE_MS`) while a moving target
  re-routes it. Every later point is sent by the clock's `step_map` as the server's copy crosses it,
  unchanged.

## Why it is not simply ASURA

ASURA `approach.rs` sends only `0x002A`, as the live capture does (`movement-live-20261109-200703.jsonl`
seq 626 -> 630), and leaves the client to walk itself there. In this server's client that walk is a
straight line: the player walked through walls and obstacles to reach an NPC, while the server's copy
walked the route. The live server may route the client some other way that is not reproduced here
(AGENTS.md rule 13). Pointing the client along the route uses the message a click already answers with.

## What follows

- The owner judges it in the live client. If the client ignores `0x0029` for its own agent during an
  approach, the alternative is to aim `0x002A` itself at the next route point.
- `docs/port/09-parity-audit.md` marks the approach's walk as **ours** for this addition.
