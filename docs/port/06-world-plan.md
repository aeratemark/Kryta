# Porting the world: what ASURA's world has, where each piece goes, and what must exist first

The infrastructure half is ported (`docs/port/01-structure.md`). What is left is the world - and the
world is not one port, it is a dependency chain. This file is the plan for it: every piece ASURA's world
is made of, the noun it belongs to here, what it needs from the schema and the content files, and the
order the chain allows.

Nothing here changes the rule the project exists for: a piece goes into the **noun** that owns it, it
reaches the database through that noun's repository, its content is a **data file**, and it never reaches
into another program.

## The chain, in order

Each stage names the ASURA sources it comes from, so a later session can read them instead of
reconstructing them.

### 1. `game/Pack/` - the content source

| ASURA | lines | is |
|---|---|---|
| `gwdat.rs` | 397 | the `Gw.dat` archive: its file table, a file by id, its chunks |
| `fa_compress.rs`, `fa_compress_tables.rs` | 368 + 139 | the archive's compression |

Nothing else can be measured without this: map blobs, models, spawns and item visuals are all file ids
into the archive. **Needs:** nothing from the schema; the archive path is a setting (`GwDatFile`).

### 2. `game/Maps/` - a map catalogue with geometry

| ASURA | lines | is |
|---|---|---|
| `map_data.rs` | 1100 | a map blob: planes, trapezoids, portals, the x/y split tree, path tables |
| `map.rs` | 104 | one map's record |
| `map_spawns.rs`, `spawns.rs` | 247 + 177 | the spawn points and the instance's own agent list |
| `cartography.rs` | 286 | the world-map blob a client asks for (`0x008A`) |

**Needs:** a **`map_config`** table in the world database (id, type, name, spawn count, region), the
**`map_spawn`** table (`map_id`, `idx`, `x`, `y`, `plane`), and a **`map_portal`** table (`map_id`,
`idx`, `x`, `y`, `to_map`, `arrive_*`). ASURA's shapes are `crates/asura-db/migrations/0020`, `0025` and
`0005` - three small tables, and the first world schema this tree writes
(`docs/decisions/0009-world-schema.md`, to be written with it).

### 3. `game/Terrain/` - walking inside a map

| ASURA | lines | is |
|---|---|---|
| `pathfind.rs`, `path_tables.rs` | 1641 + 75 | the plane walk and the path table behind it |

**Needs:** stage 2's blobs. Nothing from the schema.

### 4. `game/Agents/` - things in a map

| ASURA | lines | is |
|---|---|---|
| `agent.rs`, `agent_defaults.rs` | 1431 + 168 | an agent's identity, its properties, its appearance, the streams that describe it |
| `player.rs` | 507 | the player's own agent: the one a connection controls |
| `pvp_items.rs` | 1566 | the PvP half of a character's items |

**Needs:** the item nouns below (an agent is described by what it wears), and stage 2's spawn records.
The message family already exists (`game/Server/Packets/AgentPackets.rs`); what is missing is the state
behind it.

### 5. items - rows in the database, like every other piece of game data

An item is game data, so it is **rows**: what a definition is lives with the world, and what a
character holds lives with the character. That is this tree's rule, not a reading of another project's
layout (`docs/port/04-configuration.md`, "three kinds of data").

| piece | where | why |
|---|---|---|
| an item definition - kind, model, dyes, value, flags, name | **`item_template`** in `kryta_world` | it is content the world holds, and an operator changes it by changing a row |
| a bag, and an item a character holds (which bag, which slot, quantity, the state a player changed) | **`bag`** and **`item_instance`** in `kryta_characters` | it is state: a player's, changed by playing |
| a loot table - what a source drops and how often | **a JSON file** (`data/loot/*.json`) | it is a setting: an operator tunes it, and two servers may want different ones |

In code the reference's arrangement: the item type beside the entity it belongs to (its `Entities/Item`),
the template store in `game/DataStores/` (its `ItemTemplate`), the message builders in
`game/Server/Packets/ItemPackets.rs` (written), the handlers in `game/Handlers/` (stub). The noun that
gains behaviour is `game/DataStores/` for definitions and `game/Characters/` for what a character holds;
no new directory is invented, and no item lives in a file.

| ASURA | lines | is |
|---|---|---|
| `item.rs` | 525 | the streams and the in-memory inventory - read for the message shapes, ported into `ItemPackets.rs` and the repositories |
| `default_equipment.rs` | 2957 | the profession table, which is content: imported into `data/equipment.json` (a setting an operator may change) |

**Needs:** `item_template` in the world schema, `bag` and `item_instance` in the character schema, and
the creation confirm writing them instead of only the character row. The creation screen already sends
the same item streams from memory, so the streams are proven before the rows arrive.

### 6. `game/Districts/` - instances and how a client enters one

| ASURA | lines | is |
|---|---|---|
| `instance.rs`, `instance_table.rs` | 488 + 289 | an instance: its map, its population, its stages of loading |
| `portals.rs`, `travel_targets.rs` | 468 + 164 | where a map joins another, and which maps a character may enter |
| `travel.rs` | 511 | the travel a login records and the world claims |

**Needs:** the **`transfer`** seam (see below), stage 2's tables, and the load sequence the client reads:
`0x017C`, `0x0186`, the manifest stages, `0x0189`/`0x0188`, `0x0196` redirects. That sequence is the one
place where a port must follow the client's own order exactly, and ASURA measured it.

### 7. `game/Chat/`, `game/Parties/`, `game/Quests/` - the rest

| ASURA | lines | is |
|---|---|---|
| `chat.rs` | 340 | the chat line, the channels and the GM commands |

**Needs:** stages 4 and 6 for anything a player does while standing somewhere.

## The three seams

Everything above fits inside one noun. These three cross a boundary and each has one answer:

1. **Travel, login to world.** The login records a travel when it answers `0x8029`: the account, the
   world, the map and the player token; the world claims it once when the gameplay connection presents
   that token pair (`travel.rs`, `0004_transfers.sql`). In this tree the record is a row - the login
   database already holds accounts and a `realmlist`; a `travel` table beside them, written by the auth
   program and read by the world through a repository of its own. It is the reference's own arrangement
   (AzerothCore's worldserver reads its login database), and it keeps the two programs from needing to
   talk to each other.
2. **Characters and their parts.** The character row is written at the creation confirm and read by the
   login's list today (`docs/decisions/0007-character-schema.md`). Every stage above that adds a part - a
   bag, a setting, a last outpost - adds a table or a column, never a second repository.
3. **What is a row and what is a file.** The test is fixed and it is not the source project's: **game data
   is rows, a setting is a file** (`docs/port/04-configuration.md`, "three kinds of data"). So a map, its
   spawns, its portals, an item definition and a character's items are rows; a loot table, the profession
   sets an operator may retune, the world list and every program setting are files. A port never decides
   this per piece - it asks which of the three kinds the data is.

## The order, and what each stage proves

| # | stage | proves | blocked by |
|---|---|---|---|
| 1 | `game/Pack/` | a file id in the archive is read and its bytes checksum against the archive's own table | nothing |
| 2 | `game/Maps/` + world schema | the world hosts a real map: it answers `0x017C` with a blob from the archive and the client loads it without the creation instance | 1 |
| 3 | `game/Terrain/` | a walk from one point to another follows the blob's own planes | 2 |
| 4 | `game/Agents/` | a spawn from the map's own table appears in the client | 2, 5 |
| 5 | `game/Items/` (+ `bag`, `item` rows) | a created character's equipment is rows, and it draws from them | 2 |
| 6 | `game/Districts/` + travel | a character created in the creation instance can be sent to an outpost and arrive there | 2, 4, 5 |
| 7 | chat, parties, quests | the rest, one noun at a time | 4, 6 |

## What is already in place for this

- The **message families** the client reads: instance, agent, character, item, misc - and the walk that
  frames anything (`shared/src/Packets/`, `docs/port/02-wire.md`).
- The **socket layer** and the world listener (`game/Server/`), so a client can arrive.
- The **creation flow**, which ends in a character row and is the only path a character has today.
- The **administration tool**, which is where the world schema above will be edited from
  (`docs/decisions/0008-administration-tool.md`): its `/content` page is the map and spawn editor, and it
  arrives with stage 2.
- **The reference arrangement** for each piece: one noun, one repository, content as data.

## What this plan does not decide

- The world schema's exact table and column names: those arrive with `docs/decisions/0009-world-schema.md`
  and stage 2, and follow from the messages that read them. The **kinds** are settled and are not open:
  the world's data is rows, a setting is a file, and what the client dictates is a constant.
