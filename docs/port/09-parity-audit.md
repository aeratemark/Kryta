# Parity audit: what is a port, what is ours

Written 2026-11-12, after the map load first worked and the owner asked whether this tree is a port
at all. It is not, everywhere. This file is the honest inventory: for each piece, what this tree does,
where, what the working implementation does, and which of the two it came from.

Verdicts:

- **ported** - the code is the working implementation's, translated to this tree's structure, with
  the fact it rests on cited.
- **ported, thin** - ported, but only the part the first client run needed; the rest of the same
  subject is missing.
- **ours** - written here with no counterpart in the working implementation. Every one of these is a
  place the client can be wrong, because nothing measured it.
- **missing** - the working implementation has it and this tree has nothing.

The two sides, measured:

| | this tree | the working implementation |
|---|---|---|
| the game connection's dispatch | `game/src/Server/WorldSession.rs:78-98` (12 client messages) | `ASURA/crates/asura-world/src/dispatch.rs:23-137` (23 client messages) |
| the packet families | `game/src/Server/Packets/` (6 files, 57 builders) | `ASURA/crates/asura-protocol/src/game/mod.rs` (80 builders) |
| the agent and movement | `game/src/Agents/mod.rs` (54 lines) | `ASURA/crates/asura-world/src/agent.rs` (66 KB) + `pathfind.rs` (67 KB) |
| the login | `authserver/src/Server/{session,login_reply}.rs` | `ASURA/crates/asura-auth/src/{lib,travel}.rs` |

## 1. The connection and the load

| subject | this tree | the working implementation | verdict |
|---|---|---|---|
| the 64-byte version message and its gate | `game/src/Server/Protocol/mod.rs:124` | `ASURA/crates/asura-world/src/version.rs` | ported (ours is stricter: it closes on a world mismatch, which the reference does not) |
| the travel record claimed from the login | `WorldSession::connect`, `game/src/Maps/Travels.rs` | `ASURA/crates/asura-world/src/travel.rs` | ported, thin |
| the load head, player-data start, name, instance record | `WorldSession::connect`, `game/src/Server/Packets/InstancePackets.rs` | `ASURA/.../player.rs`, `instance.rs` | ported |
| the items answer (`0x8091`) | `creation::load_item_stream` | `ASURA/.../item.rs:70` (`items_block`) | ported, thin: the reference also sends the customized-name frames (`0x015E`, 118 in one capture), which neither implementation sends |
| the spawn answer (`0x8088`) | `WorldSession::load_map` `CLIENT_LOAD_REQUEST_SPAWN` | `ASURA/.../agent.rs:1219` (`handle_request_spawn`) | ported, thin: ASURA walks the map's arrival list; this tree sends one point of `map_spawn` plus a fixed `0x001E 500` |
| the players answer (`0x8090`) | `WorldSession::load_map` `CLIENT_LOAD_REQUEST_PLAYERS` | `ASURA/.../player.rs:205` (`player_block`) | ported (34 frames, same order) |
| **the map's own agents (NPCs, props)** | nothing | `ASURA/.../agent.rs` (creation, properties, effects, equipment), `map_data.rs` | **missing**: the map has no content because `Gw.dat` reading (`game/Pack/`) is unbuilt |
| **the cartography pair (`0x008B`, `0x008A`)** | nothing | `ASURA/.../cartography.rs`, `game/mod.rs:904` | **missing**: opening the map (U/M) will assert `worldMapDims` |
| the exploration grid, the manifest | `instance_manifest_phase/done` with fixed words | `ASURA/.../player.rs`, `item.rs` | ported, thin: the manifest's words are the trace's, not derived |
| **the instance table (`0x01A0`)** | nothing | `ASURA/.../game/mod.rs:872` (`instance_table`) | **missing** |
| **the account's unlocked-instance list** | `0x0094` with a bitmap of the character's unlocks | same | ported |

## 2. Movement - the largest divergence

| subject | this tree | the working implementation | verdict |
|---|---|---|---|
| the agent's state | `Agent { id, x, y, facing }` and `moved_to(x, y)`: a point the client's reports overwrite (`game/src/Agents/mod.rs:13-41`) | `ASURA/.../agent.rs:89` (`AgentState`: position, plane, facing, motion, waypoints, speed, move type, sync copy) | **ours** |
| `0x803E` walk to a point | read and dropped; the point is stored (`WorldSession::answer`, `movement_point`) | `ASURA/.../agent.rs:710` (`handle_move_to_coord`): a path from the map's graph, `densify` to a ~145-unit cadence, `0x0029` answers, broadcast to every other client | **ours** (no path, no answer, no broadcast) |
| `0x0029`/`0x002D`/`0x002B` and their `_live` forms | absent | `ASURA/.../game/mod.rs:1375-1475` | **missing** |
| the position report the client sends | stored | `ASURA/.../agent.rs:887` (`handle_position_update`): elapsed-gated, tolerance `50 + speed * elapsed`, and a correction when the client is further off | **ours** |
| cancel movement, last position | absent | `ASURA/.../agent.rs:1062`, `:1117` | **missing** |
| the simulation tick | `0x001E` carried a **counter** (500, 501, 502, ...) until 2026-11-12 | `ASURA/.../instance.rs:369` sends the **elapsed milliseconds** of the interval | **ours**, corrected to the port's meaning |
| the path graph | nothing | `ASURA/.../pathfind.rs` (67 KB) and the pack's `pathing` data | **missing** |
| teleport (`0x002C` `AGENT_UPDATE_POSITION`) | absent | `ASURA/.../game/mod.rs:1481`, and its note that a wrong `0x002C` disconnects the client | **missing** |

What this means for the client: it walks locally, the world never tells it anything about the walk,
and the only clock it gets is the tick - so with a counter in the tick the client's simulation runs at
a speed that changes every message, which is the "mangled walking" the owner saw.

## 3. Items

| subject | this tree | the working implementation | verdict |
|---|---|---|---|
| the inventory stream | `creation::inventory_block` | `ASURA/.../item.rs:120` | ported |
| a character's item rows | `item_instance`, whole item on the row (2026-11-12) | `ASURA/crates/asura-store/src/state/inventory.rs` (`NewItem`) | ported |
| drop an item | `0x8069` handled: the row goes, `0x014D` answered | `ASURA/.../item.rs:187` (`drop_item`), and its ground item is not sent either | ported, thin: no ground item |
| move, equip | `0x8072`, `0x8030`: the row moves, `0x013E` answered | `ASURA/.../item.rs:200-...` | ported, thin |
| unequip | refused as unmapped | `ASURA/.../dispatch.rs:114` (`handle_unequip_item`) | **missing** |
| the PvP item table | `game/src/DataStores/PvpItems.rs` | `ASURA/.../pvp_items.rs` | ported (copied) |
| storage, dyeing, merchants | nothing | `ASURA/.../item.rs`, `vendor` | **missing** |

## 4. Login, accounts, characters

| subject | this tree | the working implementation | verdict |
|---|---|---|---|
| the four portal endpoints | `game/src/Portal/` | `ASURA/crates/asura-webgate/src/lib.rs` | ported |
| the file check, the exchange | `authserver/src/Server/{file_check,handshake}.rs` | `ASURA/crates/asura-auth/src/lib.rs` | ported |
| the login answer's message order | `login_reply::reply_frames` | `ASURA/crates/asura-auth/src/lib.rs:669-746` | ported |
| the **EULA version in the account info** | passed as `0` until 2026-11-12 | read from the account row and sent (`lib.rs:610-629`) | **ours**, corrected to the port |
| the character list's settings block | `characters::CharacterSettings` (62 bytes) | `ASURA/.../messages.rs` (`character_settings`) | ported |
| where a travel request is placed | `game/src/Maps/Placement.rs` (this tree's own file) | `ASURA/crates/asura-rules/src/lib.rs` (`placement`) | ported (rules 1 and 2 only; the outpost rule is missing) |
| a travel's hand-off row | `kryta_login.travel` | `ASURA`'s transfer table | ported, thin: no leases, no capacity, no world health |

## 5. Absent whole subjects

These are not thin: they are absent, and the working implementation has them.
`chat` (`ASURA/.../chat.rs`), `portals and doors` (`portals.rs`, `travel.rs`), `pathfinding`
(`pathfind.rs`), `map data from the archive` (`map_data.rs`), `missions`, `skills and the skillbar`,
`heroes`, `henchmen`, `quests`, `guilds`, `titles`, `death and morale`, `monster AI`, `vendors`,
`storage`, `drops`.

## The order this file implies

1. **Movement**, whole: `agent.rs`'s state and its four handlers, the five frames and their `_live`
   forms, the tick's meaning, and `pathfind.rs` with the pack's path graph. Nothing of it is optional:
   the client is drawing a body the world says nothing about.
2. The cartography pair, so the map view does not assert.
3. Unequip, and the rest of the item operations.
4. The map's content (`map_data.rs`), which needs the `Gw.dat` reader this tree has not built.
5. The account's placement rules 3, and leases/capacity on a travel.
6. The absent subjects in `docs/game-rules.md`'s order.

Every row above marked **ours** is a place where the client's judgement, not a document, is the only
check. That is the cost of writing it here instead of porting it, and it is why the crashes arrived
one at a time.
