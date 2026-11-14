# The reference's structure, measured, and the order this port takes it

Read from `C:\Users\Apo\Documents\GitHub\WowWarband`, which is
`github.com/mod-playerbots/azerothcore-wotlk` at `30a36ec4d`. Every number below came from that
checkout today, not from a document about it.

## Two programs, and what each does in its own order

| program | entry point | startup order |
|---|---|---|
| `authserver` | `src/server/apps/authserver/Main.cpp` | `GetConsoleArguments`, `sConfigMgr->Configure` + `LoadAppConfigs`, `sLog->Initialize`, the PID file, `StartDB`, `sSecretMgr->Initialize`, `sIPLocation->Load`, the "mark every realm offline" update, `sRealmList->Initialize`, `sAuthSocketMgr.StartNetwork(bindIp, RealmServerPort 3724)`, then the io loop |
| `worldserver` | `src/server/apps/worldserver/Main.cpp` | the same config and logging, `ThreadPool` (2) at `:243`, `sScriptMgr->SetScriptLoader(AddScripts)` at `:272`, `SetModulesLoader` at `:273`, `sScriptMgr->Initialize` at `:282`, `sWorld->SetInitialWorldSettings` at `:318`, `StartWorldNetwork(BindIP, WorldServerPort 8085, Network.Threads 1)` at `:363`, the console and the SOAP thread, then `sScriptMgr->OnStartup` at `:398` |

On shutdown the world runs `sWorldSessionMgr->KickAll` (`:372`), `sMapMgr->UnloadAll`,
`sScriptMgr->OnAfterUnloadAllMaps` and `sScriptMgr->OnShutdown` (`:426`). This tree does the same
order: `sockets.stop()` is the kick (a flag every session reads on its next turn, then the network
closes) and the world's own shutdown is raised after it; `sMapMgr->UnloadAll` and its hook have no
work here, because a fresh world disposes the copies an earlier run left at its start.

The split is by conversation, not by database: auth answers login and hands out a world entry; the
world owns the gameplay socket.

## Libraries, and the dependency chain its CMakeLists state

| library | files | links |
|---|---|---|
| `common` (`src/common/`) | 173 | nothing |
| `database` | 49 | `common` |
| `shared` | 23 | `database` |
| `game` | 651 | `shared` |
| `scripts` | 693 | `game` |
| `apps` | 17 | `authserver` -> `shared`; `worldserver` -> `modules`, `scripts`, `game` |

`src/common/` holds 16 directories: Asio, Collision, Configuration, Cryptography, DataStores,
Debugging, Dynamic, Encoding, IPLocation, Logging, Metric, Navigation, Platform,
PrecompiledHeaders, Threading, Utilities.

`src/server/shared/` holds five: Network, Packets, Realms, Secrets, DataStores. Its `Network/` is
`AsyncAcceptor`, `NetworkThread`, `Socket`, `SocketMgr` - four files, and the reference's whole
socket layer at library level.

## Where the 16 common directories go

Ten are `common/` here: Configuration, Cryptography, Debugging, Encoding, IPLocation, Logging,
Metric, Platform, Threading, Utilities.

Six are not, and each has a named home or a named reason:

| reference | here |
|---|---|
| `Asio` | nothing: the runtime is tokio, used directly by `shared/src/Network/` |
| `Collision`, `Navigation` | `game/Terrain/`: they are geometry, which is the game's half |
| `DataStores` | `shared/src/DataStores/`, where this server's structure puts it |
| `Dynamic` | arrives with `scripts/`, when Lua scripts exist |
| `PrecompiledHeaders` | a C++ build concern, no Rust equivalent |

## The port order

Each stage ends with the reference's behaviour reproduced. Where the client can see the stage,
the client is the judge, not a test.

1. **`common/`** - Configuration, Logging, Platform, Threading, Utilities. This stage's proof is
   that both programs read their settings file and log what they read.
2. **`database/`** - the connection, statements, results, the worker pool, then `Updater/`. Proof:
   the schema applies and `schema_version` records it.
3. **`shared/src/Network/`** - `AsyncAcceptor` -> a listener, `NetworkThread` -> N workers, `Socket`,
   `SocketMgr`. Proof: a connection is accepted, assigned to the thread with the fewest
   connections, and closed cleanly.
4. **`shared/src/Secrets/`** - secret retrieval. Proof: the value is read through the same interface
   the reference uses.
5. **`authserver/`** - the stages in the order the table above gives, ending at the auth
   conversation. **This is the first stage whose bytes a client reads**, and Tyria's protocol -
   not the reference's - is what goes on the wire, and the four places below are where the
   reference keeps it.
6. **`game/Server/`** - `WorldSocket`, `WorldSession`, `WorldSocketMgr`. The world program's
   listener, inside the game library, exactly as the reference arranges it.
7. **`scripts/`** - the two loader calls the program hands the game library's script manager (`game/Scripting/`), then `Initialize`; the world raises `OnStartup` when it is ready and `OnShutdown` on the way down, and every world tick raises `OnWorldUpdate(diff)`.
8. **`game/` nouns** - one directory at a time, each its own commit.
9. **`modules/`** - the first `mod-*` crate, and the cargo member line that names it.

Outside the server there is a second deliverable: **the tools**, in `tools/`. A client has to be patched
to talk to us, a machine setting decides whether it reaches us, and a capture is what settles a wire
question no log can - each of those is ours, and `docs/port/05-tooling.md` says what is ported, from
where, and what is still borrowed.

## Where the port stands

Updated as stages land, because this is a statement of today and not of intent.

| stage | state, and what proves it |
|---|---|
| `common/` Configuration | done - both programs read their settings file, JSON, through one reader; `5ac97ba`, `30c01ec` |
| `common/` Logging | done - both programs log through the appender chain, and write the same lines to console and file; `8475380` |
| `common/` Utilities | partly - `GetPID` and `CreatePIDFile`; the rest arrives with the stage that calls it; `9087e75` |
| `common/` Cryptography | started - RC4, against three published vectors; `3463ed2` |
| `common/` Platform, Threading, the rest | not started; nothing calls them |
| `database/` settings | done - the three logical databases and the connection string, in the reference's shape; `c39113a` |
| `database/` Updater, file half | done - which `.sql` files exist, ordered by name, five states; `6a98037` |
| `database/` connection and pool | done - the pool opens, pings, and runs a deferred statement on a worker; `eab6be5` |
| `database/` Updater, apply half | done - a file runs once, is recorded with its sha1, and a changed file is refused; `22767f7` |
| `database/` the login schema | done - `realmlist` and `updates`, applied by the program itself from `data/sql/base/db_auth/`; `22767f7`, `9ebbb45` |
| `database/` the character schema | done for `characters` - applied by the world program from `data/sql/base/db_characters/characters.sql` and read by both programs; the table is this tree's own design, `docs/decisions/0007-character-schema.md`. A login before the world's first start finds no table, says so, and lists no character |
| `database/` the world schema | maps, spawns, arrivals, portals and instances are backed by world database rows; `data/sql/base/db_world/`, `game/src/Maps/`, `game/src/Districts/` |
| `shared/src/Network/` | done as a library - a client is accepted, answered and counted; `4596b27` |
| `shared/src/Packets/` | started - the byte buffer; the message families are written one file per family under `game/src/Server/Packets/`; `9f456f1` |
| the protocol tables | **ported whole and split the way the reference splits them** - 782 rows: `game/src/Server/Protocol/Tables.rs` (682, the world connection) and `authserver/src/Protocol/Tables.rs` (100, the login), written in the vocabulary of `shared/src/Packets/Types.rs` and framed by the walk in `shared/src/Packets/Frame.rs`. Proved by framing every client message of a recorded session to the size that session recorded |
| `shared/src/Worlds/` | done - the world list is read from `realmlist`, a login refuses to start on an empty one, and `data/realms.json` merges over it so an operator can state a world in a file; `7adbe37` |
| `shared/src/Secrets/` | not started - the reference's only secret is WoW's TOTP master key, so what this game's secret is has not been decided |
| `common/` Configuration | done - the reference's lookup, severity policy, environment override and masking, over **JSON** instead of the reference's `Key = value`: one handler (`Json.rs`), one hexadecimal decoder, and record lists read as rows rather than keys; `docs/decisions/0005-json-configuration.md` |
| `authserver/` | stages 1 to 8 - config, log, PID file, login database and its schema, world list, listener; it also reports its own status on a timer, because a server that cannot say whether anything reached it cannot be tested; `tests/end_to_end.rs` plays the client's login against the binary itself - the file check, the version message, the key exchange and the portal login - reads the character list it composes, asks for a map, and then connects to a spawned `worldserver` that claims the recorded travel and loads the map: the seam between the two programs, end to end |
| the file check | answered from its own constants, before the version message on the same socket: a client that asks gets its 32 bytes, or the socket closes. Payload, not an option - `authserver/src/Server/file_check.rs` |
| the auth conversation | the plaintext half, the login message read, and the five-message answer checked byte for byte against the reference trace; `13b5287`, `5802932` |
| the message stream | a table of the client's nine auth messages, because no message carries its length: a read holding four messages is answered four times. A run of this client sent them as `0x8001`, `0x8002`, `0x8000` and `0x8023` in one 76-byte segment, each named with its size, its build number and its decrypted body in the log |
| the rest of the login | the computer info (recorded, answered with nothing), the computer hash (the session info, with the salt already on the wire), the heartbeat (`0x0000` and tick 16), the EULA (answered with nothing, as the reference does), the ask-response, the player status, a character name chosen or deleted (status 0, 23 for a name the account does not hold, 2 for a failed delete - the character list is not re-sent), and the request for a map, which is answered with the world's own address in `0x0009` and an `0x0003` behind it |
| the travel record | when a login answers a map request it writes one `travel` row - account, world, map, district, language, player token - and the world claims it once by the pair its version message presents; a login without a record falls back to the version message's map. `data/sql/base/db_auth/travel.sql`, `game/src/Maps/Travels.rs` |
| which address a client dials | **measured, 2026-11-10, build 39167**: the launch address, not the one it is handed. A listener on the named port accepted nothing and the gameplay connection arrived on the auth port 35 ms later. The auth program is therefore the front door and hands it to the world; `docs/decisions/0004-gameplay-connections-arrive-at-the-front-door.md` |
| the portal | the four endpoints served over HTTP on 6601 by the auth program, answering from the login database's `account` table; `game/src/Portal/`, `authserver/src/Server/accounts.rs`, `docs/decisions/0003-portal-host.md`, `0006-accounts-are-rows.md` |
| the front door | a gameplay connection that arrives on the auth port is read for the world it names (`game/src/Server/Protocol/`), copied to that world's address from the world list (`shared/src/Network/Relay.rs`), and the world's answer carried back. A world that does not answer closes the client within 3 s rather than leaving it waiting; `authserver/src/Server/front_door.rs` |
| the account table | `account` in `kryta_login`: the login name the client types, and the pair the portal issued for it. A name it does not hold is the reference's own `400 who are you?`. Rows are inserted by an operator - creating an account is not built yet. The EULA version the client accepts (`0x8026`) is recorded on the row, which is what stops it asking again |
| `worldserver/` | stages 1 to 9 - config, log, PID file, the script manager (0 scripts, 0 hooks registered), its three databases opened and pinged (world, characters, and the login database the travel record lives in), the map catalogue loaded from the world database's own rows, the item templates seeded and read back, its listener on `BindIP`:`WorldServerPort` as world `RealmID`, and a status line on a timer that names the maps it hosts and the instances it holds open; `tests/end_to_end.rs` plays the client's whole conversation against the binary itself - creation, the load of map 148, the items, a spoken line and the chat commands, the travel request and the ticks - and is the program's own proof, and `tests/fresh_world.rs` starts the program against a database it makes and drops itself, proving the start-up a fresh machine would see (a role without `CREATEDB` makes it say so and stop) |
| `scripts/` | `AddScripts` and `AddModulesScripts`, the two loaders a program hands the script manager - the reference's `AddScripts()` and `AddModulesScripts()` (`Main.cpp:272-273`), with the shape of `custom_script_loader.cpp` for the first script. Nothing is registered yet, and the log says so - that is the result, not a placeholder |
| `game/World/` | the world a program represents: its id, the maps it can put a client in - `load_maps` reads the `map` and `map_spawn` rows of the world database at `SetInitialWorldSettings`, and the world keeps them with their spawn points - its scripts (`scripts()`, `set_scripts`), and its own tick, `update(now, diff)`, which tells the scripts (`sScriptMgr->OnWorldUpdate`, `World.cpp:1355`) and disposes the copies nobody is inside past their hold (the reference's `sWorld->Update(diff)`, `World.cpp:1131`) |
| `game/Terrain/` | the ground an agent walks on, ported whole from `ASURA/crates/asura-world/src/`: `MapData.rs` parses a map blob into `PathStaticData` (`map_data.rs`), `GraphDump.rs` reads and writes the `ASURAPG1` dump (`graph_dump.rs`), `PathTables.rs` the reference table-driven arithmetic - `PathReciproc` and `PathSqrt` as 256-entry tables of float bit patterns combined by integer arithmetic, copied from `GmPaths.c` through ASURA and checked entry for entry, because the path search costs come out of them and an FPU division would visit different trapezoids - and `Pathfind.rs` plus `Pathfind/Directional.rs` the finder itself: the BSP walk, the heap, the A* over trapezoids and portals, the corridor walk, and `PathGraphs` (one finder per map) with `load` from the archive, `from_dump`, `import` and `bind`. The world installs its graphs before its listener opens and answers `0x803E` and `0x803D` from them; a map whose blob is absent imports nothing and answers `NoGraph`, which is what the working implementation answers for a map it holds no graph for. Against this machine's archive, map 148 walks from its own spawn point |
| `game/Pack/` | the client's own archive: `GwDat::open` reads `Gw.dat`'s header, file table and table of contents (a few megabytes of 3.6 GB), and an entry is fetched by the file id a map's own record names - `0x0001B97D` for map 148. Ported from `ASURA/crates/asura-world/src/gwdat.rs`, the index half of the reference's `FaArchive.c`; against this machine's archive it holds 156,315 entries. `FaCompress.rs` is the decoder an entry needs (the reference FaCompress.c decoder: a bit window, code-length trees and an LZ block loop, with six static tables copied value for value), and against the real archive three map blobs decode to exactly the size they announce and start with the RIFF signature `ffna`. The graph built on top - the map blob's parse, its path tables and the finder - is `game/Terrain/`, and this archive is what the world imports it from at start-up |
| `game/Maps/` | the map catalogue as the database holds it - `DatabaseMaps::load` reads `map` and `map_spawn` into `MapEntry` and `MapSpawn` - the whole catalogue ASURA's pack states (265 records: 145 hostable maps, 120 destinations the client can resolve and this world cannot load, `is_hostable`), with 466 spawn points (`docs/decisions/0009-world-schema.md`), `spawns` carry the facing a character entering there faces (`map_spawn.angle`, ASURA `agent.rs:460-476`), `arrivals` reads `map_arrival` for R21's portal crossings (with the facing a crossing lands on), `arches` reads `map_arch` (`Portals.rs`: the whole of `ASURA/crates/asura-world/src/portals.rs` - a wall is crossed only when the step passes through it, a point doorway by the edges of `World.PointBoxHalfExtent`'s box, and a map is judged by its own walls and nothing else), and `DatabaseTravels` is the hand-off a login writes and a connection claims once; `TravelTargets` holds the reference's twenty `/tp` aliases, `Cartography` the exploration grid - `init_frames` builds the `0x008B` announcement and its `0x008A` slices of a Tyria grid with nothing explored, ported from `ASURA/crates/asura-world/src/cartography.rs` - and `InstanceTable` the destinations the client can resolve (`0x01A0` batches of fifteen pairs, from each map row's own `instance_value` - 137 of them); the load sends the grid and the table right after the instance's record, which is where a live server sends them (`ASURA/crates/asura-world/src/map.rs:22-58`) |
| `game/Districts/` | the running copies of maps, their population (`Peers.rs`: `PeerView`, the `introduction` and `farewell` an arrival and a departure are told with, ported from `ASURA/crates/asura-world/src/instance.rs:406-473` - so two players in one map see each other) and the district numbers the client shows: entering an outpost takes the first district with room (R22) or the one the travel named (R57), a full district is refused rather than moved (R58), a new one opens when the allotment is reached (R5, R16), an explorable opens a copy of its own (R3), and a connection gives its place back when it ends. Each copy is a row in `kryta_world.instance` (`DatabaseInstances`), holds for 15 minutes of nobody inside (R37), and is disposed by the world's tick (R40) when the hold passes |
| `game/Characters/` | the character row: `Character` and its 62-byte settings blob, and `DatabaseCharacters` - the login's list, the tool's page, and the saved position a reconnect resumes at (`Position`, `position`/`record_position`/`forget_position` from `ASURA/crates/asura-store/src/state/character.rs:213-294`), what a load writes (`last_outpost`, R14), a character's items as rows (`item_instance`: the starting set is written at the confirm and drawn by the load), and its unlocks (`character_unlock`: a new character starts with the world's first outpost, and a load only enters an unlocked map - one with no rows is written the first outpost as it loads, R12) |
| `game/DataStores/` | the item templates: `DatabaseItems` seeds one row per distinct item the creation record lists name (`data/inventory.json`, `data/equipment.json`) at start-up, reads them back into memory, and the load resolves a character's held items against them - what an item is, once, however many of it exist |
| `game/Agents/` | the player's own agent, ported from `ASURA/crates/asura-world/src/agent.rs`: `AgentState` (its position, plane, facing, the walk it is in, its last report and speed), `begin_walk`/`advance`/`cancel_movement`/`stand_still`, `adopt_report` with the `50 + speed * elapsed` budget, `adopt_correction` with the `MaxAllowedCorrection` ceiling, `densify` at the 145-unit live cadence, and `Player.rs` the load's three answers (`ASURA/crates/asura-world/src/player.rs`, 540 lines - `0x8091`'s inventory stream and map record, `0x8088`'s entry point and `0x8090`'s whole block, frame for frame, moved out of the session because the reference's player object owns them, `SendItems` and `SendInitialPackets*`); `game/Terrain/Pathfind.rs` holds the graphs the walks ask for, imported from the client archive at start-up, so a click is answered with the finder's first waypoint and a manual move walks the offset through adjacent cells; the map's own agents - the NPCs on spawn rows - arrive with step 6 |
| `game/Chat/` | the channel rule and the command table, ported from `ASURA/crates/asura-world/src/chat.rs`: `COMMANDS` (the reference's `/tp`, `/teleport`, `/maps` and `/stuck`, with the reference function each one comes from), `alias_key` and `travel_target` (the reference's `GmCommand_ToAscii` lookup in `game/Maps/TravelTargets.rs`'s twenty aliases), and `maps_answer` - the alias table `/maps` prints, one row per alias and the chat half wrapped at the reference's own hundred characters |
| `game/Handlers/` | the gameplay messages, one file per family as the reference keeps them (`src/server/game/Handlers/`): `ItemHandler.rs` (the inventory drag, the equip, the destroy and the unequip), `CharacterHandler.rs` (the creation screen's three asks and its confirm, with `Creation` and the block it is drawn from), `MovementHandler.rs` (the four movement messages), `TravelHandler.rs` (the travel request, a travel object's touch, the transfer's start, the arriving connection's landing and the `/tp` command) and `ChatHandler.rs` (a spoken line) - each an `impl WorldSession` member, with the decisions left in the nouns they belong to (`characters::equip_place`, `AgentState`, `Maps::Portals`, `place_for_travel`, `DatabaseCharacters`, `crate::creation`) and the session's surface given to them as `pub(crate)`; `docs/port/10-separation-guide.md` |
| `game/Scripting/` | the script manager, where the reference keeps it (`src/server/game/Scripting/`): `ScriptObject` (`ScriptObject.h:38`), `WorldScript` (`WorldScript.h:45`), and `ScriptMgr` with the reference's registration and lifecycle - a loader set by the program, `initialize`, a script numbered by its order of registration, a name registered twice refused, and a hook answered only by the scripts that declared it (`AddScript(script, enabledHooks)`, `ScriptMgr.h:796`). Three of the thirteen world hooks are here - `OnUpdate`, `OnStartup`, `OnShutdown`, the three moments this tree raises - and the other ten are named as ported thin in `docs/port/10-separation-guide.md`, because each needs an engine moment and argument types this tree has no counterpart for |
| `game/Server/` | the gameplay connection: the 64-byte version message (`Protocol/`), the instance, item and chat families (`Packets/`), `WorldSession` (the exchange, the creation block and the confirm - the families that follow dispatch into `game/Handlers/` - and the load in the measured order: the head block and the ping pair unasked, and a ping on the live connection answered with `0x000D` and `World.PingValue` - the load head, the player-data start, the character's name, the instance's record, the exploration grid and the instance table - then `0x8091` -> the character's items as rows and the map's record with its manifest, `0x8088` -> `0x0196`, the entry point and nothing else, the ticks coming from the ticker, `0x8090` -> the whole load block, built by `game/Agents/Player.rs`: the PvP unlocks and the item table, the account features, the unlocked lists, the skills and the progress record, the flags, the player and the agent it is drawn as with its properties and look, the party (`0x01D3`, `0x01CC`, `0x01D4`, `0x01B3`) and `0x018E`; from there the connection stays and is ticked every `World.SimulationTickMs` milliseconds, a second player's arrival is told with its player, party, agent, worn items and look (`0x0059`, `0x00B0`, `0x00B1`, `0x0020`, the unnamed items, `0x006E`, `0x0048`) and its departure with `0x005A` and `0x0021`, an inventory drag (`0x8072`) moves the item's row when the placement is one it may have and is answered with its placement, while a refused one is answered with where the item really is (`ASURA/crates/asura-world/src/item.rs:255-436`), an equip (`0x8030`) puts it in the slot its kind is worn in, takes whatever was worn there off to the first free carrying slot, and sends the agent's worn items again, a destroy (`0x8069`) removes the row and answers `0x014D`, the four movement messages (`0x803E`, `0x803D`, `0x8028`, `0x8047`) are answered by ASURA's own handlers and a tick advances the walk, a spoken line (`0x8064`) is answered with the text (`0x005D`) and who spoke it (`0x0061`) while `/maps`, `/stuck`, `/tp` and `Unknown command` are answered in the log as the reference answers them, the client's travel request (`0x80B1`) and a travel object's touch (`0x80C1`) are answered as `ASURA/crates/asura-world/src/travel.rs` answers them - the request with the eight frames of `480-518` (the effect and the stop, `0x01A6`, the redirect, the party records, and the removal of the player and the agent) off a hand-off row and a freshly drawn token, whose `is_explorable` is `World.TravelExplorable` and whose default is ASURA's `0`, and the touch with the log line alone (`132-169`) - and a step across one of the map's own walls starts that same transfer with the receiving map's arrival as the landing, which the connection the client opens next stands on and faces along (`Portals.rs`, `travel.rs:319-388`), a drop (`0x802C`) takes the stack off the ground without a word, `0x8008` ends the connection, and the load refuses a map the character has not unlocked (R12)) and `WorldSocketMgr` (the listener) |
| `tools/manage/` | the administration tool: one page over the accounts, the characters, the worlds and the content, served on loopback - a third binary, not a third server (`docs/decisions/0008-administration-tool.md`); accounts are writable, characters, worlds and content are read through the nouns' own repositories (`game::characters::DatabaseCharacters`, `shared::worlds::WorldList`, `game::maps::DatabaseMaps`) |
| `game/World/`'s `WorldConfig` | the numbers a world runs by, one field per rule that names one (R5, R13, R15, R33, R34, R37, R53, R56), read from the program's `World` section and logged at start-up |
| `game/Creation/` | the creation screen's content: the reference's six bags, the items it creates before a choice, the 32 (campaign, profession) sets, and the player-data block built from them; the data is `data/inventory.json` and `data/equipment.json`, imported from its rows by `tools/import-creation-inventory.py` |
| `modules/`, the rest of `game/` | not started |

What "done" means here is narrower than a finished server: it means the stage behaves as the
reference behaves and this tree's own tests or a real run prove it. `docs/port/02-wire.md` says what
the wire knows and what it does not.

## Where the wire lives, measured

The reference keeps its protocol in four places, and this is the arrangement the port copies:

| what | where | measured |
|---|---|---|
| the byte buffer | `src/server/shared/Packets/ByteBuffer.{h,cpp}` | 2 files |
| the packet | `src/server/game/Server/WorldPacket.h` | 1 file |
| the opcode table | `src/server/game/Server/Protocol/Opcodes.{h,cpp}`, `ServerPktHeader.h`, `PacketLog.{h,cpp}` | 5 files |
| one file pair per message family | `src/server/game/Server/Packets/` | 18 pairs: `AllPackets.h`, `PacketUtilities`, and 17 families - Bank, Calendar, Character, Chat, Combat, CombatLog, Guild, Instance, Item, LFG, Misc, NPC, Pet, Query, Quest, Totem, WorldState |

The buffer is shared, the packet and the opcode table are the game library's, and the message
structs are grouped by family rather than by direction or by connection. A single table holding
every message in both directions is the arrangement this project exists to avoid.

## Two facts that stop a careless port

- **The wire's contents cannot come from the reference.** Its message table is Azeroth's protocol.
  Tyria's - headers, sizes, field order, direction - is measured from this client, and the
  captures under `C:\Users\Apo\Documents\GitHub\ASURA\recordings\` are where that was recorded.
  The reference supplies the arrangement above and nothing about its contents.
- **`shared/src/Worlds/` is the reference's `Realms/`.** A realm there is the server a player picks;
  in this game that thing is a world. The rename is the only one in `shared/`.

## Open, and needing a word

| question | why it blocks |
|---|---|
| the five unassigned game directories (`Autobroadcast`, `Battlefield`, `Miscellaneous`, `Skills`, `Weather`) | `Skills` collides with `Spells/ -> Skills/`, so one of the two needs another name |
| whether `game/Modules/` (the reference has one, 2 files) stays separate from cargo modules in `modules/` | two things called modules in one tree |
| where a pack's configuration lives now that content is not SQL rows | the reference's `data/sql/` model does not describe `Gw.dat` |

## What is next, and what stops it

The current implementation and its evidence are in the status table above. The outstanding
client-facing work and its port order are in `docs/port/09-parity-audit.md`; the world subjects
and their dependencies are in `docs/port/06-world-plan.md`. The `Gw.dat` reader and pathfinder
are present in `game/Pack/` and `game/Terrain/`, as recorded in the status table.
