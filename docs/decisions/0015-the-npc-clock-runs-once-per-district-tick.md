# 0015 - The NPC clock runs once per district tick, whichever connection reaches it

## The reference

ASURA's instance owns a clock task (`crates/asura-world/src/instance.rs:968`) and calls
`NpcTable::tick(delta, viewers, pathing)` once per period with every entered player's position
(`instance.rs:721-728`, `:816`). The table advances every active body, ticks its tree, and returns the
messages each viewer is owed (`npc/table.rs:720-879`).

## This tree

A district has no task or clock of its own. Each gameplay connection runs the same simulation loop
(`game/src/Server/WorldSession.rs:1400-2087`), and any number of them can be in one copy.

## The decision

The copy keeps `last_tick` and the entered players' positions. `NpcDistricts::tick` records the
caller's position and advances `NpcTable` only when `SimulationTickMs` has passed since the copy's last
body tick; a connection that arrives inside that period only records its position. The elapsed time
handed on is capped at one period, so a stalled connection or a long load cannot fast-forward a body
down its route. Walk and burrow messages are queued per player and drained by that player's
`npc_sweep`, which is this tree's half of `TickOutput.deliveries` (`game/src/Districts/Npcs.rs`).

Without this, two players in one outpost would run the town twice as fast, and a twenty-player
district twenty times. The body behaviour itself is ASURA's, unchanged: the same delta, the same
`tick`, the same order of walks, trees, removes and entries.

## What it is not

It is not a new protocol rule and it does not change a message. The client sees the same `0x0029`,
`0x0028`, `0x00F1`, `0x0021` and entry bursts the table produces. The only difference is which caller
supplies the elapsed time: ASURA's instance task, or the first connection to reach the period here.
