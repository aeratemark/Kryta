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
`sScriptMgr->OnAfterUnloadAllMaps` and `sScriptMgr->OnShutdown` (`:426`).

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
7. **`scripts/`** - `ScriptMgr` and its hooks, then the world program's two loader calls.
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
| `database/` the world schema | not started - nothing in the world database is read or written yet |
| `shared/src/Network/` | done as a library - a client is accepted, answered and counted; `4596b27` |
| `shared/src/Packets/` | started - the byte buffer; the message families are written one file per family under `game/src/Server/Packets/`; `9f456f1` |
| the protocol tables | **ported whole and split the way the reference splits them** - 782 rows: `game/src/Server/Protocol/Tables.rs` (682, the world connection) and `authserver/src/Protocol/Tables.rs` (100, the login), written in the vocabulary of `shared/src/Packets/Types.rs` and framed by the walk in `shared/src/Packets/Frame.rs`. Proved by framing every client message of a recorded session to the size that session recorded |
| `shared/src/Worlds/` | done - the world list is read from `realmlist`, a login refuses to start on an empty one, and `data/realms.json` merges over it so an operator can state a world in a file; `7adbe37` |
| `shared/src/Secrets/` | not started - the reference's only secret is WoW's TOTP master key, so what this game's secret is has not been decided |
| `common/` Configuration | done - the reference's lookup, severity policy, environment override and masking, over **JSON** instead of the reference's `Key = value`: one handler (`Json.rs`), one hexadecimal decoder, and record lists read as rows rather than keys; `docs/decisions/0005-json-configuration.md` |
| `authserver/` | stages 1 to 8 - config, log, PID file, login database and its schema, world list, listener; it also reports its own status on a timer, because a server that cannot say whether anything reached it cannot be tested |
| the file check | answered from its own constants, before the version message on the same socket: a client that asks gets its 32 bytes, or the socket closes. Payload, not an option - `authserver/src/Server/file_check.rs` |
| the auth conversation | the plaintext half, the login message read, and the five-message answer checked byte for byte against the reference trace; `13b5287`, `5802932` |
| the message stream | a table of the client's nine auth messages, because no message carries its length: a read holding four messages is answered four times. A run of this client sent them as `0x8001`, `0x8002`, `0x8000` and `0x8023` in one 76-byte segment, each named with its size, its build number and its decrypted body in the log |
| the rest of the login | the EULA (answered with nothing, as the reference does), the ask-response, the player status, and the request for a map, which is answered with the world's own address in `0x0009` and an `0x0003` behind it |
| the travel record | when a login answers a map request it writes one `travel` row - account, world, map, district, language, player token - and the world claims it once by the pair its version message presents; a login without a record falls back to the version message's map. `data/sql/base/db_auth/travel.sql`, `game/src/Maps/Travels.rs` |
| which address a client dials | **measured, 2026-11-10, build 39167**: the launch address, not the one it is handed. A listener on the named port accepted nothing and the gameplay connection arrived on the auth port 35 ms later. The auth program is therefore the front door and hands it to the world; `docs/decisions/0004-gameplay-connections-arrive-at-the-front-door.md` |
| the portal | the four endpoints served over HTTP on 6601 by the auth program, answering from the login database's `account` table; `game/src/Portal/`, `authserver/src/Server/accounts.rs`, `docs/decisions/0003-portal-host.md`, `0006-accounts-are-rows.md` |
| the front door | a gameplay connection that arrives on the auth port is read for the world it names (`game/src/Server/Protocol/`), copied to that world's address from the world list (`shared/src/Network/Relay.rs`), and the world's answer carried back. A world that does not answer closes the client within 3 s rather than leaving it waiting; `authserver/src/Server/front_door.rs` |
| the account table | `account` in `kryta_login`: the login name the client types, and the pair the portal issued for it. A name it does not hold is the reference's own `400 who are you?`. Rows are inserted by an operator - creating an account is not built yet |
| `worldserver/` | stages 1 to 9 - config, log, PID file, the script manager (0 scripts, 0 hooks registered), its three databases opened and pinged (world, characters, and the login database the travel record lives in), the map catalogue loaded from the world database's own rows, and its listener on `BindIP`:`WorldServerPort` as world `RealmID` |
| `scripts/` | the hook registry content is written against: a loader, `Initialize`, and the count of what was registered. Nothing is registered yet, and the log says so - that is the result, not a placeholder |
| `game/World/` | the world a program represents: its id, and the maps it can put a client in - `load_maps` reads the `map` and `map_spawn` rows of the world database at `SetInitialWorldSettings`, and the world keeps them with their spawn points |
| `game/Maps/` | the map catalogue as the database holds it - `DatabaseMaps::load` reads `map` and `map_spawn` into `MapEntry` and `MapSpawn`, the same shape a `Map` object is assembled from (`docs/decisions/0009-world-schema.md`) - and `DatabaseTravels`, the hand-off a login writes and a connection claims once |
| `game/Districts/` | the running copies of maps and the district numbers the client shows: entering an outpost takes the first district with room (R22) or the one the travel named (R57), a full district is refused rather than moved (R58), a new one opens when the allotment is reached (R5, R16), an explorable opens a copy of its own (R3), and a connection gives its place back when it ends. The empty-hold and disposal (R37, R38, R40) arrive with the instance table |
| `game/Server/` | the gameplay connection: the 64-byte version message (`Protocol/`), the instance and item families (`Packets/`), `WorldSession` (the exchange, the creation block and the confirm, and the load: the claimed travel or the version message's map, the head block, then `0x8088` -> `0x0196` at a point of the map's own list and `0x8090` -> the character's body plus `0x018E`) and `WorldSocketMgr` (the listener) |
| `tools/manage/` | the administration tool: one page over the accounts, the characters, the worlds and the content files, served on loopback - a third binary, not a third server (`docs/decisions/0008-administration-tool.md`) |
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

A client today logs in, is listed its characters, accepts the EULA, and asks for the
character-creation instance. Its gameplay connection reaches the world, and the creation screen is
answered: the load block, the player-data block with the inventory stream, the PvP half, a profession
change, and the confirm that writes a character. What is left is a character worth entering. In order:

| next | why it is next | what stops it |
|---|---|---|
| the front door (done) - `00 05 0C 00` on 6112 is copied to the world it names | measured: this client ignores the address it is handed, so the gameplay connection arrives where the login did (`docs/decisions/0004`) | nothing; it landed |
| the world's listener and the creation block (done) - `WorldSocketMgr`, `WorldSession`, `0x017C` + `0x0189` | the creation screen cannot show without them | nothing; it landed |
| the creation screen's conversation (answered) - the player-data block (`0x8089`) now carries the inventory stream, the six bags, the profession's starting set and the player's own state; the PvP half (`0x808A`), a profession change (`0x8060`) and the confirm (`0x808B`) are answered | the screen is drawn, then asks what to draw | a profession change does not swap the equipped set: the reference takes the old pieces off (`0x014D`) and sends the new profession's set, and this world sends neither, so a client that switches profession keeps the first set on screen |
| the character schema and the confirm (done) - `characters`, written at `0x808B`, and the list read back through the login's answer | a confirmed character has to be listed, and later entered as | nothing; it landed |
| a real map: an outpost with its map blob, spawns and the instance table | a created character has nowhere to stand, and nothing else can be entered without it | `Gw.dat` reading (`game/Pack/`) is unbuilt. The order is now fixed by the owner's rules (`docs/game-rules.md`): settings first (R53), then the archive, then a map as a definition (R1), then an instance with a district number and its lifecycle (R2-R5, R32-R40), then the travel pipeline that puts a character at R13's spawn - `docs/port/06-world-plan.md`, `The next steps, in the ledger's order` |

