# Porting the world: what the functionality is, where it is described, and the order the nouns come in

The infrastructure half is ported (`docs/port/01-structure.md`). What is left is the world - and the world
is not invented here: **every in-game system is described in detail on this machine already.** This file
is the plan for it: where each system's behaviour is described, which noun it becomes, what it needs from
the schema and the settings, and the order the chain allows.

## The source of functionality: the owner's game-rules ledger

How the game works is described in **`docs/game-rules.md`** - the owner's own ledger: 58 asserted rules
(R1-R58) and 220 evidenced facts, grouped by subject - travel, districts and instances; party, heroes and
henchmen; items and commerce; quests and missions; characters, skills and movement; chat and social wire;
death, resurrection and morale; guilds and alliance; aggro and monster AI; titles and character slots;
combat numbers and skill mechanics; merchants, traders, trade and crafting. It also holds what is
contradicted, what is suspect, and what the sources do not answer.

That file is authoritative for *what the game does*. It is not a port order and not an architecture: this
file maps each subject to the noun that owns it and the order the chain forces.

Eden and Rurik are **sister projects**, not sources here: where the ledger cites one of them as the
witness for an evidenced fact, the citation stays; nothing else is read from them.
## The nouns, and which part of the ledger describes each

| system | ledger rows that describe it (`docs/game-rules.md`) | noun | reference counterpart |
|---|---|---|---|
| a map is a definition, an instance is a running copy; outposts shared, explorables per party, districts, teardown | R1-R5, R10, R16, R29, R32-R40, R57-R58 | `game/Maps/`, `game/Districts/` | `Maps/`, `Districts/` <- `Instances/` |
| where a character enters and faces when it arrives | R13, R14, R21, R22 | `game/Maps/` (spawn and arrival points) | spawn points on the map record |
| what is in a map before anyone arrives, and what respawns | R29-R31, R34 | `game/Agents/`, `game/Behaviour/` | `Agents/` <- `Entities/`, `Behaviour/` <- `AI/` |
| the travel unit: party, leader, heroes, henchmen, caps and truncation | R6-R11, R15, R17-R28 | `game/Parties/`, `game/Heroes/` | `Parties/` <- `Groups/` |
| unlocks and campaign history gating travel | R12, R47 | `game/Maps/` (unlocks per character) | `Quests/` progression |
| aggro, the compass bubble, monster behaviour | R30, R43, E11 | `game/Behaviour/`, `game/Agents/` | `Behaviour/` <- `AI/` |
| combat numbers, death penalty, morale, resurrection | R44, E10-E15 | `game/Combat/`, `game/Agents/` | `Combat/` |
| attributes, level, experience, skill points | R46, R48, R51, E13, E14 | `game/Characters/` (a character's progression) | `Characters/` |
| skills: learning, capture, the skillbar | R50, E19 | `game/Skills/` | `Skills/` <- `Spells/` |
| items: what a character holds, dyes, storage, drops | R45, R49, E9, E16-E20 | `game/DataStores/` (definitions) + `game/Characters/` (what a character holds) | `DataStores/`, `Entities/Item` |
| merchants, traders, crafting, collectors | R54, E16-E18, E20 | `game/Trader/` <- `AuctionHouse/`, `game/Agents/` (NPCs) | `Trader/` |
| quests and missions | R52, R55-R56, E15, E21 | `game/Quests/`, `game/Districts/` (missions) | `Quests/` |
| chat and the social wire | the "Chat and social wire" section | `game/Chat/` | `Chat/` |
| guilds, friends, alliance | the "Guilds, friends, and alliance" section | `game/Guilds/` | `Guilds/` |
| titles and character slots | E22, E23 | `game/Characters/` | `Achievements/` (the reference's nearest) |
| heroes as scripted behaviour | R23-R27, and ASURA `research/heroai-server-port.md` | `game/Heroes/` + `scripts/` | none - `Heroes/` is this tree's own noun |

Two subjects the ledger does not describe, and where their facts come from instead: the **archive's
file format** (`Gw.dat`) and the **wire shapes** of every message. The first is read from ASURA's archive
reader as a fact about the client's files; the second from the protocol table and the captures
(`docs/port/02-wire.md`).
## The order, and what each stage proves

The order is the chain the client itself forces: nothing can be shown before the content can be read,
nothing can be walked before a map exists, nothing can fight before an agent is somewhere.

| # | stage | ledger rows to read first | proves |
|---|---|---|---|
| 1 | `game/Pack/` | none - the archive's format is a client fact | a file id is read from the archive and its bytes checksum against the archive's own table |
| 2 | `game/Maps/` + the world schema | R1, R2, R4, R13, R14, R21, R22, R29 | the world hosts a real map: `0x017C` is answered with a blob from the archive and the client loads it instead of the creation instance |
| 3 | `game/Terrain/` | R34 (the compass range), R43 | a walk follows the blob's own planes |
| 4 | `game/Agents/` + `game/Movement/` | R29-R31, R34 | a spawn from the map's own table appears, and the player's own agent moves |
| 5 | `game/Districts/` + the travel seam | R1-R16, R32-R40, R52, R57-R58 | a character created in the creation instance is sent to an outpost and arrives where R13 says it should |
| 6 | `game/Parties/` and `game/Heroes/` | R6-R11, R15, R17-R28 | a party travels as one unit, and a cap truncates it leader-first |
| 7 | items: definitions and what a character holds | R45, R49, E9, E16-E20 | a character's equipment is rows and it draws from them |
| 8 | skills, casting and combat | R43, R48, R50-R51, E10-E15, E19 | a skill is cast on an agent and its effect is what the ledger says |
| 9 | quests, chat, guilds, titles | R52, R55-R56, E15, E21-E23 | each per its own rows |
| 10 | heroes and scripted behaviour | R23-R27 | content written against hooks, out of tree |

Stages 1-5 are the spine: without them a client can only sit in the creation instance. Stages 6-10 become
possible once 5 lands, and each can be taken in its own order.
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
