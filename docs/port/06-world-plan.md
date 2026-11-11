# Porting the world: what ASURA's world has, where each piece goes, and what must exist first

The infrastructure half is ported (docs/port/01-structure.md). What is left is the world - and the
world is not one port, it is a dependency chain. This file is the plan for it: every piece ASURA's world
is made of, the noun it belongs to here, what it needs from the schema and the content files, and the
order the chain allows. Five stages are in scope; items and the content-heavy nouns are listed at the end
with the evidence that would bring them in, because a stage with no data to run on and no capture to
judge it is not work yet.

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

**Needs:** stage 2's spawn records. An agent is described to the client by its properties and its
appearance, and both are the agent's own data - an agent wearing nothing is a valid agent, which is what
the creation screen sends today. The message family already exists
(`game/Server/Packets/AgentPackets.rs`); what is missing is the state behind it.

### Items - **out of scope**, and said here so nobody ports a skeleton (see the last section)

There is no item data in this tree: no definitions derived from the archive (stage 1 has not run), and no
capture of a live server's item stream beyond the creation screen's own block, which is already sent.
ASURA has item code - `item.rs`, `pvp_items.rs`, a generated equipment table - and porting that on its
own would be porting a skeleton with nothing to point it at: rows nobody has, for messages nobody has
recorded.

So items are deferred, and the placement below is the *rule for when the data arrives*, not work in this
plan (`docs/port/04-configuration.md`, "three kinds of data"):

| piece | where, when it lands |
|---|---|
| an item definition - kind, model, dyes, value, flags, name | `item_template` in `kryta_world`, filled from the archive |
| a bag, and an item a character holds | `bag` and `item_instance` in `kryta_characters` |
| a loot table | a JSON file under `data/loot/`, a setting |

What is **already here** is the creation screen's own stream: `game/Server/Packets/ItemPackets.rs` and
`data/equipment.json`, which exist because a creation screen is sent its starting set and we had that
capture. That is a screen's payload, not an item system.

**What would bring items in:** a recording of a live server's item traffic (equip, move, drop) - the
input a message's shape is measured from - and the archive reader of stage 1 to name the file ids those
messages carry. Until then, no table, no noun, no page.

### 5. `game/Districts/` - instances and how a client enters one

| ASURA | lines | is |
|---|---|---|
| `instance.rs`, `instance_table.rs` | 488 + 289 | an instance: its map, its population, its stages of loading |
| `portals.rs`, `travel_targets.rs` | 468 + 164 | where a map joins another, and which maps a character may enter |
| `travel.rs` | 511 | the travel a login records and the world claims |

**Needs:** the **`transfer`** seam (see below), stage 2's tables, and the load sequence the client reads:
`0x017C`, `0x0186`, the manifest stages, `0x0189`/`0x0188`, `0x0196` redirects. That sequence is the one
place where a port must follow the client's own order exactly, and ASURA measured it.

### 6. `game/Chat/`, `game/Parties/`, `game/Quests/` - the rest

| ASURA | lines | is |
|---|---|---|
| `chat.rs` | 340 | the chat line, the channels and the GM commands |

**Needs:** stages 4 and 5 for anything a player does while standing somewhere.

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
   spawns and its portals are rows; a loot table (when items exist), the profession
   sets an operator may retune, the world list and every program setting are files. A port never decides
   this per piece - it asks which of the three kinds the data is.

## The order, and what each stage proves

| # | stage | proves | blocked by |
|---|---|---|---|
| 1 | `game/Pack/` | a file id in the archive is read and its bytes checksum against the archive's own table | nothing |
| 2 | `game/Maps/` + world schema | the world hosts a real map: it answers `0x017C` with a blob from the archive and the client loads it without the creation instance | 1 |
| 3 | `game/Terrain/` | a walk from one point to another follows the blob's own planes | 2 |
| 4 | `game/Agents/` | a spawn from the map's own table appears in the client | 2 |
| 5 | `game/Districts/` + travel | a character created in the creation instance can be sent to an outpost and arrive there | 2, 4 |
| 6 | chat, parties, quests | the rest, one noun at a time | 4, 5 |
| - | items | **out of scope**: no item data and no capture of an item stream, so nothing could be verified. The rule for when it lands is below | needs both |

The numbers above are the order; items sit outside it on purpose.

## What is already in place for this

- The **message families** the client reads: instance, agent, character, misc, and the creation screen's
  own item stream - and the walk that frames anything (`shared/src/Packets/`, `docs/port/02-wire.md`).
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

## What is out of scope, and why it stays out

A stage is work only when there is **data to run it on** and **evidence to judge it by** - a capture of a
live server doing it, or the client itself. A skeleton in another tree is neither.

| out of scope | why | what would bring it in |
|---|---|---|
| items: definitions, inventory, equip, drops, loot | no item data (stage 1 has not read the archive) and no capture of an item stream beyond the creation screen's block | a recording of a live server's item traffic and the archive reader; then the rule above says where each piece lives |
| PvP: heroes, skills, the PvP item table | nothing recorded of a PvP conversation, and no data for the tables | a capture of a PvP session |
| quests, achievements, guilds, parties beyond their message rows | no data and no capture | content plus a recording |

The message tables already hold those families' rows (they are read from the protocol, not invented), and
framing them costs nothing. Having a *row* for a message is not having its noun: the noun arrives when
data and evidence do.