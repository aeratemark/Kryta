# Separation of server infrastructure and the Guild Wars world

This guide records the code inspected on 2026-11-14. It identifies the boundaries already in place,
where responsibilities overlap, and what must be examined to complete the intended separation.
It does not establish a new design or change client behaviour. The design authority is the local
AzerothCore checkout at `30a36ec4d`; ASURA at `ca9f532` supplies client-facing behaviour.
`docs/port/01-structure.md` records what runs, and `docs/port/09-parity-audit.md` records protocol
parity. Each proposed change needs its own comparison with those authorities.

## Current boundaries

| Boundary | Status | Evidence |
|---|---|---|
| Infrastructure and game | Established at crate level | `Cargo.toml` and each crate's `Cargo.toml`: `common`, `database`, and `shared` do not depend on `game`; `game` depends on them. |
| Login and gameplay processes | Established, with code coupling | `authserver/src/main.rs` starts the auth listeners. `game/src/Server/WorldSocketMgr.rs` starts the gameplay listener for `worldserver`. Auth depends on `game` as described below. |
| Game mechanics and gameplay connection | Partial | `game/src/Agents/mod.rs`, `game/src/Terrain/`, and `game/src/Maps/Portals.rs` contain mechanics. `game/src/Server/WorldSession.rs` also owns gameplay decisions and state. |
| Game engine and scripts | Not yet operational | `scripts/src/ScriptMgr/mod.rs` stores script names and hook counts. The loaders are empty and `game/src/Scripting/mod.rs` says it is not ported. No gameplay callback dispatch is implemented there. |
| Modules | Not yet implemented | `modules/README.md` states that there are no module crates; `scripts/src/AddModulesScripts.rs` is empty. |

The AzerothCore reference deliberately places its gameplay socket inside the game library. A socket
in `game/Server/` is therefore consistent with the reference's structure. Process boundaries and
crate dependencies also do not establish that all responsibilities inside a crate are separated.

## 1. Auth depends on the game library

**What exists.** `authserver/Cargo.toml` includes `game`. The auth program hosts the identity portal
implemented in `game/src/Portal/`, as recorded in `docs/decisions/0003-portal-host.md`. The gameplay
front door reads a game version message and relays the connection to the world
(`authserver/src/Server/front_door.rs`; `docs/decisions/0004-gameplay-connections-arrive-at-the-front-door.md`).
The world owns the gameplay session.

**Where the boundary is wider.** `authserver/src/Server/session.rs:1076` calls the game map-placement
rule, and `:1168-1179` builds and records the travel hand-off. The session also uses game account and
character types. Decision 0003 describes the portal as the one game module used by auth, so that
statement no longer describes all the current uses. AzerothCore's `src/server/apps/CMakeLists.txt:130-139`
links auth to `shared` and world to `modules`, `scripts`, and `game`.

**Work to resolve.** Inventory each auth use of `game`, including the portal, character data,
placement, travel and routing. Compare the ownership and hand-off with AzerothCore and the required
ASURA conversation. Decide where each responsibility belongs and record any departure from the
reference in `docs/decisions/`. Preserve the client's messages and their order, and update the parity
ledger for any listed change. Hosting an HTTP listener and choosing the ownership of its code are
separate decisions. The correct destination for each piece has not been established by this guide.

**Evidence of completion.** The dependency graph and actual imports agree with the resulting design,
the hand-off and login tests pass, and a real client verifies the login and entry sequence. Any
remaining deviation is described accurately in its decision record.

## 2. Gameplay session owns several mechanics

**What exists.** `game/src/Server/WorldSession.rs` holds connection handling and also carries gameplay
work: the portal crossing (`cross_portal`), the map's clock (`step_map`, which calls `Agent::advance`
and `Portals::crossed` and decides when to advance and what to do with the result), and the travel and
chat families, which have not moved yet.

**Done (2026-11-14, item family).** The item messages are the first family moved, all four of them:
`answer_item_move`, `answer_equip_item`, `answer_drop_item` and `answer_unequip_item` are
`impl WorldSession` members in `game/src/Handlers/ItemHandler.rs`, and the
decision they used to hold - where an item would be worn - is `characters::equip_place` in
`game/src/Characters/Equipping.rs`, a pure function over the character's rows, the item templates and
the equipped bag, which prints nothing and reads nothing. `move_item`, `equipped_item_ids` and
`first_free_carrying_slot` were already the noun's. No frame, order, size or log line changed; the
end-to-end item journey (drag, equip, drop, unequip) passes untouched. The session exposes what a
handler needs (`say`, `send`, `characters()`, `bags()`, `item_templates()`, `equipped_bag()`,
`player_id()`) as `pub(crate)`, which is the surface every later family will use.

**Done (2026-11-14, movement family).** The four movement messages moved the same way:
`answer_move_to_coord`, `answer_position_update`, `answer_cancel_movement` and `answer_last_position` are
`impl WorldSession` members in `game/src/Handlers/MovementHandler.rs`, and the dispatch in the session is
one call each. Their decisions were already the nouns': `AgentState::adopt_report`,
`begin_walk`/`begin_manual_walk`/`advance`/`cancel_movement`/`adopt_correction` in `Agents/`, the route
in `pathfind`/`directional`, and the wall in `Maps/Portals.rs`. What stays in the session is what is not a
message: the map's clock (`step_map`) and the transfer it can start (`cross_portal`), which the guide's
third and fourth items own. No frame, order or size changed; the end-to-end journey's walk, its doorway
crossing and its tick measurement all pass untouched.

There is already useful extraction: agent state and movement operations are in `game/src/Agents/`,
pathfinding in `game/src/Terrain/`, portal geometry in `game/src/Maps/`, and character item persistence
in `game/src/Characters/`. The presence of these modules does not make every session operation a
separate mechanic.

**Reference.** AzerothCore separates message-family handlers under `src/server/game/Handlers/`.
For example, `ItemHandler.cpp:155-265` handles the request and calls player operations including
`CanEquipItem` and `EquipItem`. Its `WorldSession` still has responsibilities, so individual
operations need comparison before assigning them elsewhere.

**Work to resolve (remaining families).** Trace each implemented session path from packet through state
change and reply.
For each subject, compare the matching AzerothCore handler and game objects for ownership, then the
ASURA handler for the complete Guild Wars behaviour and message sequence. Place the resulting
responsibilities in the reference's corresponding layers. Preserve all branches, order, persistence,
error handling and frames. Moving a function to another file alone does not settle its ownership.
Start with a specifically selected subject and keep the work and parity audit scoped to it.

**Evidence of completion.** The subject has a traceable handler-to-mechanic path, the mechanic's
state and decisions have a clear owner, existing callers and persistence still work, and recorded
frames and the client confirm the same behaviour.

## 3. Simulation and lifecycle ownership

**What exists.** The session advances its player's agent in `WorldSession::step_map`. Separately,
`worldserver/src/main.rs:647` runs a thread that sweeps empty districts. `game/src/World/mod.rs`
holds shared world data and districts. These are real pieces of a simulation, distributed between
the session, application and game library.

**Reference.** AzerothCore's `src/server/apps/worldserver/Main.cpp:618` calls `sWorld->Update(diff)`;
`src/server/game/World/World.cpp:1131` implements the world update and calls the script world-update
hook at `:1355`.

**Work to resolve.** Trace Kryta's current tick and lifecycle paths, then compare their ownership and
execution order with the reference and their observable behaviour with ASURA. Determine which update
responsibilities belong in the world, its maps and instances, and the session. Account for shared
state, concurrent sessions, disconnects and shutdown before changing scheduling.

**Evidence of completion.** A trace shows which component schedules and updates each implemented
piece, instance lifecycle and movement remain correct with multiple connections, and client-visible
results match ASURA. Automated tests support this check; the client is the judge of visible behaviour.

## 4. Script and module boundary

**What exists.** `worldserver/src/main.rs:151` initializes `ScriptMgr`. The registry holds a mapping
from script name to a number of hooks (`scripts/src/ScriptMgr/mod.rs`), and `AddScripts.rs` and
`AddModulesScripts.rs` register nothing. It cannot currently store and dispatch gameplay callbacks.
`game/src/Scripting/mod.rs` and `game/src/Modules/mod.rs` are unported. No module crate exists.

**Work to resolve.** Port the relevant AzerothCore script manager interfaces and lifecycle at the
locations the reference uses, including an actual route from engine events to registered callbacks.
Establish how scripts and modules register against those interfaces. For Guild Wars behaviour, use
ASURA and `docs/game-rules.md` as the behaviour sources. The owner selected Lua for external game
behaviour, with Rust providing simulation mechanics, in ASURA `docs/30-world-content-and-scripting.md`
(recovered under `docs/evidence/asura-history/`). The integration and content interfaces still need
to be designed against Kryta's structure.

**Evidence of completion.** A registered script can receive an implemented engine event and affect
an authorized game action through the intended interface. A module can register and receive its
hooks. Lifecycle, ordering and client-visible behaviour have corresponding verification.

## Applying the guide

These items are findings and work to investigate, rather than authorization to alter the protocol or
choose a new architecture. Follow `AGENTS.md` before each change, including its ASURA comparison,
pre-flight sentence, single-subject and audit requirements. `docs/port/06-world-plan.md` maps game
systems to their owners and `docs/game-rules.md` records their expected behaviour. Reassess this guide
when a boundary changes, and correct outdated descriptions where they already occur.
