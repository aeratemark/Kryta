# Porting the world: what the functionality is, where it is described, and the order the nouns come in

The infrastructure half is ported (`docs/port/01-structure.md`). What is left is the world - and the world
is not invented here: **every in-game system is described in detail on this machine already.** This file
is the plan for it: where each system's behaviour is described, which noun it becomes, what it needs from
the schema and the settings, and the order the chain allows.

## The three sources of functionality

| source | holds | measured |
|---|---|---|
| **Eden** `C:\Users\Apo\Documents\GitHub\Eden`, `docs/re/` | the client's own in-game systems, one document per system: movement, collision, combat, skills, skillbar, chat, quests, inventory, items, guilds, parties, maps, districts, animation, text, camera, the UI wire | **135 documents, 6.07 MB** |
| **Rurik** `C:\Users\Apo\Documents\GitHub\Rurik` | the game's data model and a mod kit: `content/*.toml` (areas, maps, npcs, items, drops, quests, world, movecode, partycap, fog), `schema/messages.json` (**409 KB**), and **61 `studies/`** subjects - combat, skills, skillcast, castmech, monsterai, morale, quests, movement, terrain, mapdata, mapload, maprows, worldmaps, minimap, heroes, profession, itemmods, weapons and the rest | `content/` 12 files, `studies/` 61 subjects |
| **ASURA** `C:\Users\Apo\Documents\GitHub\ASURA`, `docs/` | the wiring between them: `guidance/architecture.md`, `guidance/scripting-and-behavior.md`, `guidance/game-packs.md`, `work/world-simulation-plan.md` (its seven phases), `reference/instance-initialization.md`, `reference/inventory-model.md`, `reference/map-catalogue.md`, `reference/client-protocol.md`, `research/heroai-server-port.md`, `research/skill-mechanics-and-behavior.md` | the documents this tree already cites, plus the ones it does not yet |

**The rule this file takes from that:** functionality is **read** from those documents, the *arrangement*
of a system comes from the reference (AzerothCore: which noun owns it, which manager drives it, how a
service is arranged), the *placement of data* comes from this tree's test (rows for the world's state, a
file for a setting, a constant for what the client dictates), and the *bytes* come from the captures. A
system is not invented, and it is not copied from another project's file layout.

## The nouns, and which document describes each

| system | described in | noun | reference counterpart |
|---|---|---|---|
| the archive: file ids, chunks, compression | Rurik `studies/archivewrite`, `datwrite`, `templates`, `unitwrite`; Eden `asset-loading`, `binary-extraction` | `game/Pack/` | none - `Pack/` is this tree's own noun |
| a map: blob, surfaces, chunks, spawns, portals | Eden `map-chunks`, `map-surfaces`, `map-bloat`, `map-drawing`, `world-map`, `map-altitude-query`; Rurik `studies/mapdata`, `mapload`, `maprows`, `terrain`, `minimap`, `worldmaps`; ASURA `reference/map-catalogue.md` | `game/Maps/` | `Maps/` |
| geometry and walking | Eden `collision`, `collision-props-agents`, `pathfinding-collision-scope`; Rurik `studies/terrain`, `movecode`, `movement` | `game/Terrain/` | `Terrain/` <- `Grids/` |
| things in a map: identity, properties, appearance, streams | Eden `agent-movement`, `agent-facing`, `agent-animation-state`, `agent-attr-updates`, `agent-text`, `agent-model-chain`, `keyboard-movement`; Rurik `studies/agentprops`, `unitassembly`, `unitmodels` | `game/Agents/` + `game/Movement/` | `Agents/` <- `Entities/`, `Movement/` |
| instances and how a client enters one | ASURA `reference/instance-initialization.md` (the live sequence, in order), `guidance/architecture.md` (travel end to end); Eden `district-list`; Rurik `studies/customarea`, `presearing`, `isle` | `game/Districts/` | `Districts/` <- `Instances/` |
| items, inventory, drops | Eden `inventory-storage`, `const-data-inventory`, `item-description`, `item-drop`, `packet-inventory`; Rurik `content/items.toml`, `content/drops.toml`, `studies/itemmods`, `weapons`; ASURA `reference/inventory-model.md` | `game/DataStores/` (definitions) + `game/Characters/` (what a character holds) | `DataStores/` (`ItemTemplate`), `Entities/Item` |
| skills, casting, combat, enemy behaviour | Eden `skills-window`, `skillbar`, `skillbar-interaction`, `skill-tooltip`, `skill-upkeep`, `combat-hud`, `enemy-skill-indicator`; Rurik `studies/skills`, `skillcast`, `castmech`, `combat`, `monsterai`, `morale`; ASURA `research/skill-mechanics-and-behavior.md` | `game/Skills/`, `game/Combat/`, `game/Behaviour/` | `Skills/` <- `Spells/`, `Combat/`, `Behaviour/` <- `AI/` |
| chat and commands | Eden `chat.md`; Rurik `studies/chat`, `cmsg` | `game/Chat/` | `Chat/` |
| quests and missions | Eden `quest-log-window`, `quest-and-mission-ui`; Rurik `content/quests.toml`, `studies/quests` | `game/Quests/` | `Quests/` |
| parties and guilds | Eden `party-window`, `guild-window`; Rurik `content/partycap.toml` | `game/Parties/`, `game/Guilds/` | `Parties/` <- `Groups/`, `Guilds/` |
| heroes and scripted behaviour | ASURA `research/heroai-server-port.md`, `guidance/scripting-and-behavior.md`; Rurik `studies/heroes`, `monsterai` | `game/Heroes/` + `scripts/` | none - `Heroes/` is this tree's own noun |

That table is the plan's spine: a stage is **one system**, its descriptor is named, and its noun is fixed
before any code is written.

## The order, and what each stage proves

The order is the chain the client itself forces: nothing can be shown before the content can be read,
nothing can be walked before a map exists, nothing can fight before an agent is somewhere.

| # | stage | descriptor to read first | proves |
|---|---|---|---|
| 1 | `game/Pack/` | Rurik `studies/archivewrite`, `datwrite`; Eden `asset-loading`, `binary-extraction` | a file id is read from the archive and its bytes checksum against the archive's own table |
| 2 | `game/Maps/` + the world schema | ASURA `reference/map-catalogue.md`; Eden `map-chunks`, `map-surfaces`; Rurik `studies/mapdata`, `mapload` | the world hosts a real map: `0x017C` is answered with a blob from the archive and the client loads it instead of the creation instance |
| 3 | `game/Terrain/` | Eden `collision`, `pathfinding-collision-scope`; Rurik `studies/terrain`, `movecode` | a walk follows the blob's own planes |
| 4 | `game/Agents/` + `game/Movement/` | Eden `agent-movement`, `keyboard-movement`; Rurik `studies/agentprops` | a spawn from the map's own table appears, and the player's own agent moves |
| 5 | `game/Districts/` + the travel seam | ASURA `reference/instance-initialization.md` | a character created in the creation instance is sent to an outpost and arrives there |
| 6 | items: definitions and what a character holds | Eden `inventory-storage`, `const-data-inventory`; ASURA `reference/inventory-model.md` | a character's equipment is rows and it draws from them |
| 7 | skills, casting and combat | Rurik `studies/skillcast`, `castmech`, `combat`; Eden `skillbar`, `combat-hud` | a skill is cast on an agent and its effect is what the descriptor says |
| 8 | chat, quests, parties, guilds | Eden `chat.md`, `quest-log-window`; Rurik `content/quests.toml` | each per its own descriptor |
| 9 | heroes and scripted behaviour | ASURA `research/heroai-server-port.md`, `guidance/scripting-and-behavior.md` | content written against hooks, out of tree |

Stages 1-5 are the spine: without them a client can only sit in the creation instance. From 6 on, each
stage is independent of the ones beside it and can be taken in any order once 5 lands.

## The three seams

Everything above fits inside one noun. These cross a boundary and each has one answer:

1. **Travel, login to world.** The login records a travel when it answers `0x8029` - the account, the
   world, the map, the player token - and the world claims it once when the gameplay connection presents
   that pair. In this tree the record is a row: a `travel` table beside `account` and `realmlist`, written
   by the auth program, read by the world through a repository of its own. That is the reference's own
   arrangement (its worldserver reads its login database) and it keeps the two programs from talking to
   each other.
2. **Characters and their parts.** The character row is written at the creation confirm and read by the
   login's list (`docs/decisions/0007-character-schema.md`). Every later stage that adds a part - a bag, a
   skill, a quest state, a last outpost - adds a table or a column, never a second repository.
3. **What is a row and what is a file.** Game data is rows; a setting is a file; what the client dictates
   is a constant (`docs/port/04-configuration.md`, "three kinds of data"). A port asks which of the three
   the data is - never where another tree happened to keep it.

## What is already in place

- The **message families** and the walk that frames anything (`shared/src/Packets/`, `docs/port/02-wire.md`).
- The **socket layer** and the world listener (`game/Server/`), so a client can arrive.
- The **creation flow**, which ends in a character row.
- The **administration tool** (`tools/manage/`), where the world schema will be edited from: its
  `/content` page is the map, spawn and item editor, and it arrives with stage 2.
- The **protocol table**, which already holds the rows of every family above - a row is not a noun, but it
  is the framing a noun needs.

## What this file does not decide

- The world schema's table and column names: those arrive with `docs/decisions/0009-world-schema.md` and
  stage 2, and follow the messages that read them.
- Which of the documents above is authoritative where two disagree: the client is the judge, and the
  disagreement is reported (`AGENTS.md`, rule 3).
