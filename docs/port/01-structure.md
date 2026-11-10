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
| `database/` world and character schemas | not started - the tables are Tyria's and are a decision, not a port |
| `shared/src/Network/` | done as a library - a client is accepted, answered and counted; `4596b27` |
| `shared/src/Packets/` | started - the byte buffer; the opcode table and the message families are not written; `9f456f1` |
| `shared/src/Worlds/` | done - the world list is read from `realmlist`, a login refuses to start on an empty one, and `data/realms.json` merges over it so an operator can state a world in a file; `7adbe37` |
| `shared/src/Secrets/` | not started - the reference's only secret is WoW's TOTP master key, so what this game's secret is has not been decided |
| `common/` Configuration | done - the reference's lookup, severity policy, environment override and masking, over **JSON** instead of the reference's `Key = value`: one handler (`Json.rs`), one hexadecimal decoder, and record lists read as rows rather than keys; `docs/decisions/0005-json-configuration.md` |
| `authserver/` | stages 1 to 8 - config, log, PID file, login database and its schema, world list, listener; it also reports its own status on a timer, because a server that cannot say whether anything reached it cannot be tested |
| the file check | answered from its own constants, before the version message on the same socket: a client that asks gets its 32 bytes, or the socket closes. Payload, not an option - `authserver/src/Server/file_check.rs` |
| the auth conversation | the plaintext half, the login message read, and the five-message answer checked byte for byte against the reference trace; `13b5287`, `5802932` |
| the message stream | a table of the client's nine auth messages, because no message carries its length: a read holding four messages is answered four times. A run of this client sent them as `0x8001`, `0x8002`, `0x8000` and `0x8023` in one 76-byte segment, each named with its size, its build number and its decrypted body in the log |
| the rest of the login | the EULA (answered with nothing, as the reference does), the ask-response, the player status, and the request for a map, which is answered with the world's own address in `0x0009` and an `0x0003` behind it |
| which address a client dials | **measured, 2026-11-10, build 39167**: the launch address, not the one it is handed. A listener on the named port accepted nothing and the gameplay connection arrived on the auth port 35 ms later. The auth program is therefore the front door and hands it to the world; `docs/decisions/0004-gameplay-connections-arrive-at-the-front-door.md` |
| the portal | the four endpoints served over HTTP on 6601 by the auth program, answering from the login database's `account` table; `game/src/Portal/`, `authserver/src/Server/accounts.rs`, `docs/decisions/0003-portal-host.md`, `0006-accounts-are-rows.md` |
| the account table | `account` in `kryta_login`: the login name the client types, and the pair the portal issued for it. A name it does not hold is the reference's own `400 who are you?`. Rows are inserted by an operator - creating an account is not built yet |
| `worldserver/` | stages 1 to 4 - config, log, PID file; stops at its databases; `b604814` |
| `game/`, `scripts/`, `modules/` | not started |

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

A client today logs in, is shown an empty character list, accepts the EULA, and asks for the
character-creation instance; then it dials the gameplay connection at the address it was launched
with, and nothing can answer it. In order:

| next | why it is next | what stops it |
|---|---|---|
| the front door - the auth program accepts a `00 05 0C 00` connection on 6112 and copies it to the world's address | measured: this client ignores the address it is handed, so the gameplay connection arrives where the login did (`docs/decisions/0004`) | nothing; it is the next build |
| the world program's listener (`game/Server/`: `WorldSocketMgr`, `WorldSocket`, `WorldSession`) | it is the only thing that can answer a gameplay connection | the reference starts `StartWorldNetwork` after `sWorld->SetInitialWorldSettings`, and the world program still stops at its databases - stages 5 to 8 must land first, with an empty content tree that reports zero maps |
| the gameplay connection's protocol: the 64-byte version message, the token pair the front door issued, the seed exchange, then the creation instance's load block | the creation screen cannot progress without it | a travel has to be recorded for the world to check, which the front door brings |
| content: the creation instance first (no geometry), then one outpost with a map blob, then spawns and the `Gw.dat` reader (`game/Pack/`) | the world serves maps, and has none | `Gw.dat` reading is unbuilt |
