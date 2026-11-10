# AGENTS.md

Binding rules for any agent working in this repository. Read this file before the first edit of
a session.

The one sentence: **AzerothCore's server, in Rust, serving Guild Wars.**

A new session starts here, then reads `docs/port/01-structure.md`: what runs today, what proves it, and
what the next piece is. Every claim there is a statement of today, updated in the same commit as the
change that moves it.

## The task

Port AzerothCore into Rust - the same structure, the same services, the same arrangements - with
Guild Wars in place of Azeroth.

| | |
|---|---|
| the design authority | `C:\Users\Apo\Documents\GitHub\WowWarband`, `github.com/mod-playerbots/azerothcore-wotlk` at `30a36ec4d`. Where a file goes, what a program starts, how a layer is arranged, what a service owns: this tree answers it. Its measured structure is `docs/port/01-structure.md`. |
| the game | Guild Wars. Tyria's client, protocol, nouns and content replace Azeroth's. |
| consulted per question | `C:\Users\Apo\Documents\GitHub\ASURA` at `ca9f532`, a previous server for this client, declared failed. Opened only when one specific in-game fact is needed - what this client sends, expects or does. Never for design, structure, process or planning. Its documents describe its own function; they are not rules here. |

When the reference and another source disagree about design, the reference wins. When the question
is what the client sends or expects, the client wins, and the captures in `recordings/` are where that
was measured - three kinds of capture, and which layer of the protocol each one holds, is
`docs/port/02-wire.md`, "Three kinds of capture"; reading the wrong one as the wire is how a header
doubles.

A port means the reference's structure, behaviour, field order, sizes and order of operations,
translated into Rust. It does not mean the reference's comments, its C++ idioms, or its Azeroth
content.

## The rules

1. **The reference decides design.** An arrangement it has is the answer. A deviation is written in
   `docs/decisions/`, with its reason, in the same commit that takes it.
2. **Two halves never share a commit.** Infrastructure - processes, sockets, packets, database,
   secrets, the script manager, modules, configuration - and game mechanics - agents, terrain, maps,
   spawns, movement, combat.
3. **The client is the judge.** Where a running client and a document disagree, the client wins, and
   the disagreement is reported rather than split.
4. **Do exactly the command.** A measurement or an explanation is evidence for reading the reference;
   it is not permission to change behaviour.
5. **One change, one subject.** The commit message is the sentence you would say out loud.
6. **A stage is done when it works** - the reference's behaviour reproduced, and the client's
   judgement wherever the client can see it. Not when the tests pass, and not when the log looks
   right.
7. **Settings live in a JSON file.** A setting is read from the file, never compiled in; a program's
   file and what belongs in it are `docs/port/04-configuration.md`.
8. **A fact names where it came from.** A number or a byte string that came from the client cites the
   recording or document it came from, where it is used. A reference decision cites `file:line`.
9. **What the program does, it says.** Every stage reports itself in the log, and a trace can show
   what a client sent, so a fault is read rather than guessed - `docs/port/03-tracing.md`.
10. **Every file is ASCII.** Write `-`, `->`, `=>`, `"`, `'`, `...`, `#`. Never rewrite a file with a
    `Get-Content` / `Set-Content` round trip.
11. **A move inside this tree is additive.** Copy into the new home, re-export at the old path, delete
    the old copy, in one commit. The compiler is then the complete list of callers.
12. **Say the short true thing.** A correction replaces the text it corrects; it is never appended
    beside it, in a file or in an answer.

### Before any change: the pre-flight sentence

One sentence: what changes, in which message or code path, and why it is not simply copying the
reference. If it contains anything the command did not ask for, stop and ask. "I believe" is not a
source.

## Why the separation is the point

This project exists because the previous attempt at this client's server fused the two halves into
one arrangement that could not be separated afterwards, and rebuilding it in place failed. So the
structure is not decoration around working code. Two programs, a shared core, a database layer, a
game library by noun with its socket layer inside it, scripts written against hooks, modules out of
tree, one config file per program - that separation is the deliverable. A commit that blurs it is a
regression against the whole project, whatever it makes work.

## The tree

The reference's `src/` is flattened into the top level, and its dependency order is kept - read
from its CMakeLists files at `30a36ec4d`:

```text
common <- database <- shared <- game <- scripts

authserver links shared;   worldserver links shared, game, scripts
```

| directory | is | reference |
|---|---|---|
| `common/` | the library both programs link | `src/common/` |
| `database/` | connection, statements, results, the worker pool, the schema updater | `src/server/database/` |
| `shared/` | `Network/`, `Packets/`, `Worlds/`, `Secrets/`, `DataStores/` | `src/server/shared/` |
| `game/` | one directory per noun of the game; the socket layer lives in `game/Server/` | `src/server/game/` |
| `scripts/` | `ScriptMgr` and `AddScripts`: content written against hooks | `src/server/scripts/` |
| `modules/` | `mod-*`, out of tree | `modules/` |
| `authserver/`, `worldserver/` | the two programs, each with its own config file | `src/server/apps/` |
| `data/` | the content tree and its updater input | `data/` |

The socket layer is not a program. Sockets and sessions live in `game/Server/`, which is why one
process owns one listener and still keeps the network as a subsystem.

## Where the wire lives

The reference does not keep its protocol in one table, and the split it uses is the one this tree
uses. Measured in its `src/server/`:

| what | where |
|---|---|
| the byte buffer | `shared/Packets/ByteBuffer.{h,cpp}` |
| the packet | `game/Server/WorldPacket.h` |
| the opcode table | `game/Server/Protocol/Opcodes.{h,cpp}`, `ServerPktHeader.h` |
| one file pair per message family | `game/Server/Packets/`: `AllPackets.h`, `PacketUtilities`, and 17 families - Bank, Calendar, Character, Chat, Combat, CombatLog, Guild, Instance, Item, LFG, Misc, NPC, Pet, Query, Quest, Totem, WorldState |

One table holding every message in every direction is the arrangement this project exists to
avoid.

## The naming

The reference names its directories after Azeroth's nouns. `game/` holds 41 here; the reference
holds 50.

**24 keep the reference's name:** Accounts, Achievements, Addons, Cache, Calendar, Chat, Combat,
Conditions, DataStores, Events, Globals, Handlers, Misc, Modules, Motd, Movement, Pools,
Reputation, Scripting, Server, Texts, Time, Tools, World.

**9 are the same directory renamed:** Agents <- Entities, Terrain <- Grids, Districts <-
Instances, Parties <- Groups, Skills <- Spells, Drops <- Loot, Behaviour <- AI, Trader <-
AuctionHouse, Support <- Tickets.

**3 keep their own names:** Maps, Guilds, Quests.

**5 have no reference directory:** FileCheck (archive and version exchange before login), Portal
(identity, party and storage over HTTP), Pack (the `Gw.dat` reader), Heroes, Cinematics.

**7 reference directories are dropped with a reason, not silently:** Battlegrounds,
ArenaSpectator, Warden, Mails, Petitions, OutdoorPvP, DungeonFinding have no Tyrian counterpart.
`Vehicles` is dropped with them and is not a directory: it is a class inside `Entities/`.

**5 reference directories are unassigned and are not in this tree:** `Autobroadcast`,
`Battlefield`, `Miscellaneous`, `Skills` (which collides with `Spells/ -> Skills/`), `Weather`.
`PrecompiledHeaders` and `TC9Sidecar` are C++ and tooling concerns and do not cross. Each
unassigned directory needs the owner's word before it appears.

## How to state progress

What runs today, and what proves it, is the table in `docs/port/01-structure.md`. Read it before
claiming anything: it is a statement of today, updated in the same commit as the change.

A stage that has not landed stops with a reason and exit code 1 instead of printing that it ran. No
claim of a stage is made without the command that proves it, and where a client can see the stage,
the client is the judge rather than a test.

## Where things are

| need | path |
|---|---|
| the reference's measured structure, and the port order | `docs/port/01-structure.md` |
| the wire facts this tree uses, and what it does not know yet | `docs/port/02-wire.md` |
| what the server reports, and how to switch on its traces | `docs/port/03-tracing.md` |
| every setting, where it is set, and what is not a setting | `docs/port/04-configuration.md` |
| the tools: the client patch, the hosts setting, the imports, and what is still borrowed | `docs/port/05-tooling.md` |
| the settings reader | `common/src/Configuration/`, and `Json.rs` in it for the format |
| a decision taken away from the reference, and why | `docs/decisions/` |
| workspace, dependency order, lint policy | `Cargo.toml` |
| the byte buffer | `shared/src/Packets/` |
| the opcode table | `game/src/Server/Protocol/` |
| one file pair per message family | `game/src/Server/Packets/` |
| the login conversation | `authserver/src/` |
| the game program | `worldserver/src/` |
| the hook registry | `scripts/src/ScriptMgr/` |
| one config file per program | `authserver/authserver.json.dist`, `worldserver/worldserver.json.dist` |
| this machine's cargo | `C:\Users\Apo\.cargo\bin\cargo.exe` |
