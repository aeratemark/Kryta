# History

Finished work, kept as it was written: parity-era audits, delivered plans, research that a decision closed.
Nothing here is current truth. Where a section disagrees with the code, the code is right.
References inside a section are as written on its date; a few name files that no longer exist, or never did.

## What we learned from GWPS (research record)

> From `docs/history.md`, moved here on 2026-11-06.


> **GWPS is not a migration target.** ASURA is written from scratch, following AzerothCore's
> structure (`docs/adr/0006-azerothcore-is-the-reference.md`). This document records what GWPS
> taught us and where the reference code for each fact lives. The "steps" below describe the
> shape the new stack must have, not edits to the old project
> (`docs/adr/0005-gwps-reference-only.md`).

> **But when the user says "port it from GWPS", GWPS is copied verbatim.** That is a standing
> directive, not a design discussion: `AGENTS.md` #2. The user has the
> final say on what a feature must do; a port follows the source's code and bytes exactly, even
> where this document's own reasoning would choose otherwise. The `/tp` incident
> (`docs/HANDOFF.md`, "Why the command-is-the-requirement rule exists") is the worked example:
> GWPS's exact struct is what worked.

GWPS is not being thrown away. It is the working reference: every protocol fact, every client
quirk, and every "this made the client fail with error 007" was discovered there. This document
maps what to reuse as knowledge, what to port, and what to leave behind.

## What GWPS proved (keep as established facts)

- The client can be pointed at a private server: DH keys patched, mutex check disabled, launched
  with an auth server on loopback.
- Login, character list, character selection, and entering a map all work.
- Travel between maps works: request -> redirect (`0x01A6`) -> map update (`0x0099`) -> reconnect with
  a token that the server validates.
- Maps load from `Gw.dat`: the archive index, the map blob format, spawn points.
- The cost of a map server is measurable and small: ~12 MB, one thread, one database connection.
- A hop hangs when the destination server is built inside it; a hop is reliable when everything is
  prebuilt. That single experiment is the reason this repository exists.

## Component map

| GWPS file | what it becomes |
|---|---|
| `AuthSrv.c` | **auth service**: auth endpoint, handshake, login, character list, sessions, allocation decisions. Ported to Rust. |
| `AuthSrvCtrl.c`, `CtrlConn.c/.h` | **database**: retired. Placement and state coordination move into tables (`world`, `world_pool`, `session`), so the live control messages have nothing left to carry. |
| `main.c` | split: `asura` main and `world` main. Today one binary binds 6112/6113/1337 and runs everything. |
| `GameSrv.c` | **world server**, ported to Rust in `crates/asura-world`: connection, load block, movement, travel, chat, items. It is no longer C and no longer outside this project. |
| `GmMapDataImport.c`, `FaArchive.c`, `GmPaths.c`, `GmItem.c`, ... | **world server**, ported: the archive importer, decompressor and A* live in `crates/asura-world` (`map_data.rs`, `fa_compress.rs`, `pathfind.rs`, `item.rs`). |
| `Db.c`, `DbSchema.h` | **database**: ported from SQLite to PostgreSQL and split into `auth` / `game` / `content`, owned by `asura-db`. Character writes move from a connection per instance to a pooled async queue in each world server. |
| `GmCommands.c` | **world server**, plus the map list it carries is the input for outpost prebuild. |
| `tools/patch-gw.py`, `keys.txt` | **`client-patcher/`** in this repository, verbatim: the client patcher and the DH parameters the server copied. Nothing client-facing runs without them. |
| `tools/hosts-gwps.ps1` | **`client-patcher/hosts-redirect.py`**: the hosts-file companion, ported to Python so it is one command on Windows and Linux. It is the opt-in redirect, not a default: a plain patched client is live, and the block sends the six names (or all seven with `--file-server`, the content address included) to this machine. |
| `tools/`, `captures/` | test material for the port: packet traces to diff against. |

## What must not be inherited

These are the structures that cap GWPS, and carrying them over would recreate the same ceiling:

- **One thread per map instance.** Replace with a reactor and a fixed worker pool.
- **One SQLite connection per instance.** One per world server, behind a queue.
- **A private copy of the `Gw.dat` index per instance.** One per world server, memory-mapped.
- **A map server constructed on demand inside a travel.** Pools, prebuilt, assigned.
- **Sockets handed between components.** Each world accepts its own connections and does its own
  gameplay handshake.
- **Everything in one process.** The control plane is a separate program.

## Migration steps

Each step is independently testable against the real client, and each one leaves the system
working. Do not start a step before the previous one is proven.

### Step 1 - A world server that owns its connections

Give the game server its own listener and its own gameplay handshake, instead of having the auth
program accept the socket and pass it over. The client is told `127.0.0.1:<world port>` in the
redirect instead of the auth port. Nothing else changes.

- Uses: the same handshake code, now running in the world server.
- Test: log in, hop, on one machine, same client, same moves as GWPS.
- Why first: this is the one genuinely risky change, and everything else depends on it being true.

### Step 2 - Split the programs

`asura-auth` and `asura-world` become separate binaries. The world server registers itself in the
`world` table and publishes pool capacity; placement moves into the world server, arbitrated by the
database, instead of being decided inside the login process.

- Test: two processes, two terminals, same hops; kill the world mid-session and confirm the client
  fails cleanly rather than hanging.
- Acceptance: the auth service contains no `GameSrv` code and never reads `Gw.dat`.

### Step 3 - Capacity and refusal

The auth service knows how full each world is and can answer "no". The travel request path gains the refusal
branch, delivered before the client is redirected.

- Test: configure a world with one explorable slot, travel into it from two characters, confirm the
  second is refused *while still standing in the outpost*.

### Step 4 - Cheap instances inside a world server

Shared index, one database connection per world, reactor instead of a thread per instance.

- Test: 500 instances in one world server; measure memory and thread count against the numbers in
  `docs/02-architecture.md` (the targets are ~5 MB and a constant thread count).

### Step 5 - More machines

Because step 1 made the client-facing address real, this is configuration: run a world on another
host, point it at the auth service, verify a hop across hosts.

- Test: one world on the same machine and one on another; travel between them; the client should
  not notice anything except a longer loading screen.

## Order of risk

| step | risk | why |
|---|---|---|
| 2 | low | mechanical once the pieces are separate |
| 3 | low | our own code path, no client involvement |
| 4 | low | internal, measurable |
| 5 | medium | addresses, firewalls, latency - no protocol change |
| 1 | **high** | the gameplay handshake moves; failure looks like an opaque client error |

Step 1 is high risk *and* first, which is not a contradiction: it is the only one that cannot be
deferred, because it decides whether the rest of the architecture is possible. It is also the most
testable, since the client tells you immediately when it is wrong.

## What the GW2 microservices talk changes in this design

> From `docs/history.md`, moved here on 2026-11-06.


Source: *Guild Wars Microservices and 24/7 Uptime*, Stephen Clarke-Willson (ArenaNet), GDC 2017.
References below are to slide numbers, so any claim can be checked against the deck.

This is a comparison, not a summary. For each point: what they do, whether ASURA already does it, and
what we should change. The second half of the talk - restartability - is where we were weakest.

## Already covered

| their design | ASURA | note |
|---|---|---|
| Login separate from world simulation (slides 12-13: client -> Patching, Login, Map servers) | auth service vs world servers | same split, fewer services because we have fewer concerns |
| Many small services with one job each (slide 11, ~80 types on slide 15) | auth service, world servers, database | we are deliberately coarser; we have no commerce, tournament, social or web estate |
| Self-registration plus configuration (slide 30: map servers self-register; many services register with RouteSrv) | `world` registry table written by each world | ours is the "registration" half; the "configuration" half is the `world_pool` capacity rows |
| Orchestration is assignment of services to hardware, permissions, routing, isolation (slide 25) | placement = registry + pools | we have no permissions layer; see "Not adopting" |
| The game itself is a *macroservice*, not a microservice (slide 18) | world server | same conclusion: maps are big, stateful and not independently restartable |
| Maps are time-sliced over cores inside a blade (slide 20) | one update task per map, no pool yet | the same idea is the intended shape but **not built**: `crates/asura-world/src/instance.rs:376-385` starts one task per map, and the bounded pool with map affinity is `docs/32` phase 3 |
| Stable identity instead of physical addressing (slide 22: they moved from IP+port+ProgramId+build+map+PlayerId to "ProgramId implicit + player or group GUID") | **partly** - we have tokens, but see "Identity" below | they went further than we have |

## Adopted: the rules we were missing

### 1. Trust only the most recent connection (slides 49-50)

Their failure story: A is connected to B; A dies without a clean close; B waits for a TCP timeout; A
reconnects *before* the timeout expires; B now believes there are two connections and only the newer
one works. Their rule: **only ever honour the most recent connection from a machine or service**, even
if the older one looks alive and has work in flight.

This is exactly our situation, and we have already been bitten by its cousin. Our client drops its
connection and reconnects on **every travel**, so "a new connection arrives while the old one has not
been reaped yet" is the normal case, not the exception. GWPS handled it with an ad-hoc registry check
and a special case that cleared "a transfer that never completed" - three bugs came from that area
(error 007 on a rejected reconnect, the replayed transfer, the disconnected-during-transfer case).

**Rule for ASURA:** when a player authenticates a new connection, the new connection wins
unconditionally. Kill or ignore any older session for the same account/character; never reject the
new one because an old one still looks alive. Lease release follows the same rule: the newest session
owns the lease, and the older one's release is a no-op.

### 2. Restartable means "rebuild by asking", not "reload from disk" (slides 39-46)

Their argument, which is the most useful thing in the talk for us:

- Local state in RAM is a highly optimised cache; it is usually **not authoritative**.
- Persisting semi-transient state to disk and reloading it after a restart is usually wrong, because
  time passes while you are down and the state has moved on.
- The pattern that works: keep authoritative state in RAM and, on restart, **re-query the parties that
  know** - Heroes' Ascent's tournament server keeps *no state at all* between restarts and rebuilds by
  asking each map who is in it.
- Where state must survive, keep **two copies that do not restart together** (WvW: load balancer + maps,
  either may restart as long as not both; plus a rarely used emergency copy).

Applied to us, this is a correction to how we wrote `02-architecture.md`:

| state | we said | what it should be |
|---|---|---|
| sessions | in ASURA's memory, lost on restart | keep in memory, rebuild by asking worlds; a lost session costs a re-login, which is acceptable *because we say so* |
| pool leases | `world_pool.leased` is the truth | the **world server** is the truth about what it has handed out; `world_pool` is the reservation ledger that makes concurrent hops safe. On restart, ASURA reconciles `leased` from world reports instead of trusting the table |
| player position and inventory | saved in `game` | still saved (it must survive a crash), but understood as *durable*, not *authoritative in real time*; a restart loses the last tick, never the character |
| world registry rows | written by worlds, read by auth | unchanged, and it is the model the talk endorses |

The extra rule that falls out: **never design a state such that two components must restart
simultaneously to stay correct.** Leases are the place where this could go wrong, so the reconciler
above is not an optimisation - it is what makes a restart safe.

### 3. Identity should not be physical (slide 22)

They moved routing from `IP + port + ProgramId + build + map + PlayerId` to `ProgramId implicit +
player or group GUID`, precisely because a transient identifier (a per-map player id) breaks when the
player moves. We currently carry `server_id + player_token`, both transient.

**Rule for ASURA:** the durable identity is the **account/character GUID and the party GUID**. Server
ids, tokens and ports are *routing hints* with a lifetime, never something another component keys on
for correctness.

### 4. Retry policy: retry timeouts, fail everything else fast (slides 47-48)

A restart means 10-20 seconds offline, which is why retries matter more than failover for short
outages. Their rules:

- retry **only** on timeout; any other error is reported and the transaction abandoned;
- 4-6 automatic attempts, roughly 30 seconds apart;
- application knowledge earns exceptions (their patcher retries forever);
- exponential backoff with jitter at the application layer is described as wasted effort that adds
  unpredictability.

**Rule for ASURA:** the travel path retries **nothing** - a timeout produces a refusal, because the
client is standing in an outpost and must not be sent to a server that may not be there. Idempotent
database work may retry per the rules above. Login may retry a little; the client's own reconnect
loop is the outer retry.

### 5. Metrics are cheap, and counters must be transformed (slides 31-37)

They collect about a million sequenced values per minute across both games; `AtomicAdd`-style counters
are cheap; a separate service aggregates; Graphite for display. The sharpest point is slide 36-37: an
accumulative counter that resets on restart *hides the rate*. They publish the derivative instead.

**Rule for ASURA:** every world server and the auth service exposes a small fixed set of counters
(instances, leases, players, rejected travels, DB queue depth, tick latency) and a **rate** form for
anything cumulative. Restarts must not make a graph lie.

### 6. Deploy in parallel and drain (slides 19, 20, 21)

`SpawnSrv` starts a new build, routes new traffic to it, verifies it does not fall over, then shuts
the old one down. Several builds run concurrently, and the map macroservice runs several versions at
once.

**Rule for ASURA:** the registry carries a **build id** and a **draining** flag. Placement sends new
players to the newest healthy build; a draining world keeps its players and accepts nobody new.
Nothing else in the system needs to know a build happened.

## Not adopting, and why

- **~80 services.** Their service count is a consequence of a decade of product surface (commerce,
  tournaments, social, web). ASURA has one game and one small territory. Splitting into 80 services
  would buy isolation we do not need at a cost of coordination we would have to pay daily. The
  lessons above are about *rules*, not counts.
- **Their own hardware, no virtualisation** (slides 26-27). They pin processes to IPs and let the OS
  scheduler do the work, on hardware they own and have kept stable for years. That is a legitimate
  choice with a large capital commitment; the talk itself says it is not a general recommendation
  against virtualisation (our transcription's conclusion 9). Our equivalent of "stable hardware" is
  modest: a box, a database, and processes.
- **DNS-based service announcement** (slide 30: "in an ideal world, self-registration and DNS would be
  enough - that is not our reality yet"). We do the boring thing they actually do: a registry that is
  written by the servers and read by their consumers.
- **A separate DB service layer** (their `DbSrv` / `DbSqlSrv`). They front databases with services, so
  that caching, retries and observability have a home. We do this inside `asura-db` rather than as a
  network hop. If retry policy or metric volume ever justifies it, it becomes a service.

## What this does not answer

The talk has nothing to say about our client's peculiar demand - that a travel must land on an
already-running instance - because their player never reconnects per map change the way ours does.
That remains our deviation, and it stays documented in `docs/adr/0004-port-azerothcore-structure.md`.

## Status: what works, what is a stub

> From `docs/history.md`, moved here on 2026-11-06.


History: this page was generated by `tools/parity-report.py`, retired when parity closed (docs/16).
It is a record, not a live table: a table typed by hand is a status table that is wrong in a month.

It compares this repository with the GWPS reference's own dispatch switches, so *missing* means
"the reference acts on this message and we do not".

## Implemented handlers

32 of the reference's client-message handlers are present in this repository.

## No-ops in the reference itself

The reference's handler ignores every parameter and does nothing a client can observe.
Implementing these would be inventing behaviour, so they are a deliberate gap rather than
missing work:

- `GAME_CMSG_ITEM_UPDATE_EQUIPPED_COLOR` (`0x8084`) -- GameSrv_HandleChangeEquippedItemColor ignores all three parameters (UNREFERENCED_PARAMETER) and returns ERR_OK

## Missing, with real behaviour in the reference

Nothing.

## Known partial areas

Handlers are the floor, not the whole of parity. These are implemented *and* limited, stated so
that nobody reads the table above as more than it says:

- **movement** pathfinds: `MOVE_TO_COORD` is answered with the first waypoint of this server's own path
  out of the map's graph (`crates/asura-world/src/pathfind.rs`). A walk the finder refuses is cancelled
  and **nothing is sent**: the reference broadcasts `AGENT_UPDATE_POSITION` there, and that message
  disconnected a live client once (capture seq 519) and snapped it into a wall once, both measured
  (`crates/asura-world/src/agent.rs:722-743`). Whether this silence is the port or a deviation is the
  owner's open ruling; `crates/asura-world/tests/load.rs:3169` is the test held for it;
- **world state** is players and their movement: several clients in one map are introduced to each
  other, see each other walk, and are told when one leaves
  (`crates/asura-world/src/instance.rs`, `PeerView` and `introduction`;
  `crates/asura-world/src/world.rs:166`). There are no NPCs, no combat and no quests;
- **instances** are built at startup from `map_config` -- every map in it by default (main towns,
  explorable areas and the character-creation instance), as the reference pre-spawns one server per
  configured map. `--maps` narrows the set on purpose and is never required;
- **items** are rows (`asura_db::inventory`), and the block sent for them is the reference's own
  down to the byte (`crates/asura-world/src/lib.rs`, the byte-equality test against the capture).
  What works: dropping (with the stack actually deducted, which the reference never does), moving
  an item between slots, and wearing one. What does not: buying, selling, salvaging, identifying,
  splitting a stack by hand (a drop splits; nothing else does), and **picking anything up** -- the
  reference has no pick-up handler and this one has none either, so a dropped item is litter for
  the rest of the session. `DROP_ITEM` is the one item message the reference's dispatch acts on;
  its handler is unfinished there (a `TODO` for partial stacks, no removal) and this port finishes
  it -- a recorded deviation, argued in `docs/history.md`. What a drop *sends* is
  **nothing**, and that is not configurable: `ITEM_REMOVE` (`0x014D`) and an item agent (`0x0020`
  with `AgentType_Item`) are both in no capture and a live client crashed on each of them. Dropping
  still deducts, and the client does its own subtraction -- measured. A move is answered with
  `ITEM_MOVE_TO_LOCATION` (`0x013E`), which is in the capture. `ITEM_MOVE` and `EQUIP_ITEM` are
  messages the reference does not dispatch at all, so their behaviour here is this project's;
  `UNEQUIP_ITEM` (`0x804F`) is not implemented because its layout carries no item id and no
  capture has shown what it means (dragging an item out of the equipped bag arrives as
  `ITEM_MOVE` and works);
- **persistence** saves where a character stood when it leaves and puts it back there when it
  returns (`asura_db::game::record_position`). That is a thing the reference does *not* do -- it
  keeps a character's last map and re-spawns at the map's own spawn point every session -- so a
  character's position is a deliberate addition, recorded in `0006_inventory.sql`. The inventory is
  durable too, because it is rows: a drop, a move and an equip all survive a relog. Nothing else
  about a character is saved yet: no level, no profession, no gold, no quest state;
- **character creation** (the four `CHAR_CREATION_*` handlers) is implemented. The creation instance is
  hosted (`MapType_CharacterCreation`, `crates/asura-world/src/main.rs:237-251`), all four messages are
  dispatched (`crates/asura-world/src/dispatch.rs:65-71`), and the profession's default equipment is
  carried as data from the reference's `GmDefaultArmors.c`
  (`crates/asura-world/src/default_equipment.rs`);
- **the game connection's front door** is a byte-copy router; the reference hands the socket
  itself to the game server, which needs one process and is not what this does
  (`crates/asura-gateway/src/relay.rs`).

## Chat and chat commands, command by command

`GAME_CMSG_SEND_CHAT_MESSAGE` counts as *implemented* above because this repository has code for it,
which is a lower bar than this table uses. What the two servers actually do with a typed line, read
out of `GmChat.c` and `GmCommands.c`:

| typed | the reference | here |
|---|---|---|
| `!text` | `Channel_All`, sent (`0x005D` + `0x0061`) | the same two messages |
| `#text` | `Channel_Group`, sent | the same two messages, on the group channel |
| `$text` | `Channel_Trade`, sent | the same two messages, on the trade channel |
| `@`, `%`, `"` | ignored | ignored |
| `/stuck` | cancels the movement and broadcasts the agent's position (`0x002C`) | the same |
| `/maps` | prints its table **in its own log**; its chat answer returns before the wire | prints the same table, line for line, in its own log; **nothing on the wire** |
| `/tp <map>`, `/teleport <map>` | asks the auth service for a world and tells the client to reconnect | the same cycle, world-initiated: the seat is reserved for the destination, the transfer row is written and `TRANSFER_GAME_SERVER_INFO` then `MAP_UPDATE_CURRENT` are sent (`crates/asura-world/src/travel.rs:181`, reached from `crates/asura-world/src/chat.rs:339`). The alias has to name a map this process hosts |
| anything else after `/` | `Unknown command`, in its log (its sender returns before the wire too) | `world: Unknown command`, in this log |

**A spoken line is sent; a generated answer is printed -- in both servers.** Every server-generated
answer in the reference funnels through `GameSrv_SendSystemChatMessage` or
`GameSrv_SendInvalidCommand`, and both return before touching the wire unless `GWPS_CHAT=1`; a
player's line is not gated either way. So the reference's `/maps` is useful because it prints on the
*server*, and this one prints in the same place, in the same format, and puts no byte on the wire --
which is the same crash that made the reference's layouts settle: `0x005D`, `0x005E` and `0x0061`
appear in no capture, and sending one here crashed a live client. `/tp` acts in both servers now: the
same world-initiated travel -- a slot reserved, a transfer row written, `TRANSFER_GAME_SERVER_INFO`
and then `MAP_UPDATE_CURRENT` sent -- implemented at `crates/asura-world/src/travel.rs:181` and
reached from the chat command at `crates/asura-world/src/chat.rs:339`.

## Features the reference has but keeps switched off

Parity with GWPS includes matching what it *disables*, not only what it implements. From its own
source:

- **server-generated chat** is gated in the reference (`GameSrv_ChatSendingEnabled`, `GWPS_CHAT=1`)
  and this project has no such variable: its answers are printed and not sent, which is what the
  reference does with the gate off, and is therefore the same observable behaviour with nothing
  behind a variable. Its comment records that an unconditional send put a message on the wire the
  client could not parse, **and that it crashed the client**; this port tried the same thing and a
  live client crashed on `!hello` too. A player's own line is not gated in either server -- it is
  sent -- so the split is: spoken goes out, generated is printed;
- **GM commands**: the reference has `/tp`, `/teleport` and `/maps`, and all three are here. `/maps`
  and `/stuck` because they can only describe what this world actually runs; `/tp` and `/teleport`
  because the world starts the transfer cycle itself (`crates/asura-world/src/travel.rs:181`). The
  limit is the alias: it has to name a map this process already hosts.

## The inventory model, and what measuring it changed

> From `docs/history.md`, moved here on 2026-11-06.


`docs/08` used to be a plan: three tables, an item block built from rows, `DROP_ITEM`, and a position
that survives a logout. This is what of it is now true, and - more usefully - the four places where the
reference disagreed with the plan. Two of them are in `GmInventory.c` and were settled by reading it; one
is in `msgpack.c` and was settled by counting the bytes in the capture; and one is only visible in the
capture's ordering.

## What exists

| piece | where |
|---|---|
| `bag` and `item`, the item-id sequence, and `characters.pos_plane` | `crates/asura-db/migrations/0006_inventory.sql` |
| reading a character's bags and items, seeding the default set, finding the item a drop names | `crates/asura-db/src/inventory.rs` |
| the seed itself: six bags and nine items, kept as rows from the capture | `crates/asura-db/src/inventory/seed.rs` |
| `CREATE_NAMED_ITEM`, `ITEM_MOVE_TO_LOCATION`, `ITEM_SET_PROFESSION` builders (the captured bytes are kept as a recording under `recordings/`) | `crates/asura-protocol/src/game/mod.rs` |
| the item block built out of rows, the wearing ids read from the equipped bag | `crates/asura-world/src/item.rs:70` (`items_block`) and `:120` (`inventory_block`); `crates/asura-world/src/player.rs:145` (`player_block`) |
| `DROP_ITEM` (`0x802C`) | `crates/asura-world/src/item.rs:187` (`drop_item`), dispatched at `crates/asura-world/src/dispatch.rs:73` |
| the position saved on the way out and read on the way in | `asura_db::game::record_position` / `position`, `Standing::resume` |

**The proof that this is the reference's inventory and not an approximation of it**: the block built from
six bag rows and nine item rows is compared to the captured messages - every message, in order, byte for
byte (`the_item_block_from_rows_is_the_capture_byte_for_byte`). The inventory did not go from "replayed" to
"rebuilt and close enough"; it went from replayed to rebuilt and *identical*, and the test says so on every
run.

## What the measurement changed

The plan said three things that the reference does not do. Each one is now behaviour, a test, and a line
in the status document, rather than a guess someone would have to rediscover.

1. **A drop does not take the item out of the bag.** `docs/08` said "find the item, remove it from its
   slot, create the world item, broadcast". `GameSrv_HandleDropItem` finds the item, checks the count,
   creates an agent for it at the player's position plus (30, 30) and returns. It never touches the bag,
   never frees the item id, and never sends `ITEM_REMOVE` - so a dropped cupcake is a second cupcake
   standing on the ground while the first stays in the backpack. The function is plainly unfinished (its
   partial-stack branch is a `TODO` in its own comment).
   **This port finishes it instead of copying it** - `crates/asura-db/src/inventory.rs`,
   `take_from_stack` - because a *measured* live session settled the argument: the client sends
   `DROP_ITEM` with `count = 1` for a stack of a hundred, so the reference's rule (`count` must equal the
   whole stack) means the reference's own default cupcake cannot be dropped **at all**, and a live client
   asked three times and got nothing. Copying that is copying a dead end.
2. **An item agent's model id is the item's id**, not the item's model id (`item_agent->model_id =
   item->item_id`), which is why the live session saw nothing on the ground once a drop did get through.
   This port sends the item's `model_id`; the reference's line is kept in the handler's comment so the
   difference is not a mystery.
3. **`CREATE_NAMED_ITEM`'s modifier count is one byte, not two.** Every other counted field in both
   message families is a `u16`; `TYPE_NESTED_STRUCT` in the reference's own packer writes a `u8`
   (`msgpack.c`). That is why the cupcake's message is 36 bytes and `sizeof(GameSrv_CreateNamedItem)` is
   425 - the same trap `docs/03-protocol.md` records for the auth messages, one field further in.
4. **The bags go out in model order, not id order.** The capture's bags are ids 1, 2, 6, 3, 4, 5, because
   the reference walks its `GmBagArray` (backpack, material storage, unclaimed, storage, equipped) and the
   ids were handed out in a different order. Sorting the rows by `bag_id` produces a stream that differs
   from the reference's in message order alone - which `tools/compare-load.py` would have reported as an
   ordering fault in a block whose bytes were all correct.

**And two things that are deliberately not the reference's**, both now implemented and both recorded in
`docs/history.md`:

- **moving an item and wearing one.** `GAME_CMSG_ITEM_MOVE` (`0x8072`, measured: `{item_id u32, bag_id
  u16, slot u8}`, the inverse of the `ITEM_MOVE_TO_LOCATION` we send) and `GAME_CMSG_EQUIP_ITEM`
  (`0x8030`) are messages the reference does not dispatch at all - its whole client-message list is 18
  cases and none of them touch an item. So this behaviour is this project's: a move is checked against
  the bag, the slot and its occupant, and a refused move is answered by telling the client where the item
  really is, because the client moves it in its own model before it asks. Wearing something moves it into
  the equipped-items bag slot its `ItemType` belongs to (`game::equipped_slot`) and re-sends
  `UPDATE_AGENT_VISUAL_EQUIPMENT`.
- **a character's position.** The reference keeps a character's last *map* (`characters.last_outpost`) and
  re-spawns it at that map's spawn point on every session; it never stores a coordinate.
  `0006_inventory.sql` stores one, the world writes it when the player leaves and reads it when the
  player returns, and `Standing::resume` uses it only in the map it was taken in. `pos_plane IS NULL` is
  what "never saved" means, because (0, 0) is a legal coordinate.

## The traps this left behind

- **A message nobody has measured will crash a client, and three did - one per guess.** `ITEM_REMOVE`
  (`0x014D`) and the item re-creation crashed it; the ground agent (`0x0020` with `AgentType_Item`)
  crashed it; `!hello` crashed it. None of `0x014D`, the item-agent form of `0x0020`, `0x005D`, `0x005E`
  or `0x0061` appears in the capture, and the reference's own comments predicted the chat one exactly
  (*"sending chat crashed the client twice ... every message we send is a guess"*). What survived is what a
  capture or a client measured: a drop that sends **nothing** (the client does its own subtraction, and
  the stack went down in a live session with no message at all), and chat that sends nothing unless
  `ASURA_CHAT=1`. **The rule, in one line: a message that is in no capture waits for one.**
- **The move's confirmation is still open, and both answers were reported.** With the echo (`0x013E`,
  which *is* in the capture - eight times, in the inventory block) a live client moved the item; with
  silence, the item sprang back and the move did nothing. So the echo is the default. The same session
  also reported a clone, but that build was re-creating items on a drop, so the clone's cause is not
  settled: `ASURA_ITEM_MOVE=silent` and `ASURA_DROP=none` separate them in two runs.
- **A connection that panics must say so.** The accept loop reaps each connection and prints the panic's
  own message, because a task that dies of a panic leaves the process alive, so from outside "the server
  crashed" and "one connection panicked" look identical.
- **An item id is a runtime allocation, not a row number.** The reference hands them out with
  `GameSrv_AllocateItem` and a client only needs them to be consistent within a session. ASURA writes them
  from `item_id_seq`, so the seed's capture ids (1..9) are renumbered as they are written and two
  characters never share one. A fresh database seeds the first character with 1..9 and therefore
  reproduces the capture exactly - but no test depends on that, because a test that depends on a sequence
  is a test that fails on the second character.
- **A bag's own item is a row with no slot.** The reference's `items` table is keyed by
  `(account_id, char_id, bag_model_id, slot)`, so it cannot store one at all; it rebuilds the backpack
  from a struct compiled into the server (`g_Backpack`). Here it is an ordinary item row with `bag_id` and
  `slot` NULL, and `bag.assoc_item_id` names it - which is why the schema has a CHECK that both are NULL
  or neither is, and a *partial* unique index on the slot.
- **`GAME_CMSG_POSITION_UPDATE` is 26 bytes, not 12.** `{position, plane, unk0, unk1}`; the world reads
  the first three fields and the framer still wants all 26. A test that writes 12 bytes leaves the
  connection waiting for the rest of a message it never finishes, and looks exactly like a hang.
- **The spawn answer is where the agent is, not where the map says to arrive.** `GameSrv_HandleInstance
  LoadRequestSpawn` sends `agent->position`, so a client that asks after it has walked gets the walked
  position back - and it is the same number `WORLD_CREATE_AGENT` carries. That was invisible while the
  server kept no position at all; it is the first thing that makes a stored position observable.
- **There is no "this stack changed" message.** Neither family has one, so a stack that loses something
  is removed from the client's model (`ITEM_REMOVE`, `0x014D`) and re-created at what is left, in the slot
  it was in. `GameSrv_SendBagItems` is the same rebuild - create, then move - which is where the pattern
  comes from.
- **The client moves an item in its own model before it asks.** A refused `ITEM_MOVE` that answers with
  silence leaves the two sides disagreeing until the next inventory reload, so a refusal sends
  `ITEM_MOVE_TO_LOCATION` for where the item actually is.

## What is next

The four `CHAR_CREATION_*` handlers have landed since: the creation instance is hosted and the default
equipment is carried as data (`crates/asura-world/src/main.rs:237-251`, `dispatch.rs:65-71`,
`default_equipment.rs`), so nothing of the reference's dispatch is left (`docs/history.md`).

What the inventory still cannot do, in the order a player would notice it: **pick a dropped item back up**
(`GAME_CMSG_INTERACT_ITEM`, `0x803F` - the reference has no handler, so this is a measurement away: drop
one, interact with one, read the header out of the trace); split a stack by hand
(`GAME_CMSG_ITEM_SPLIT_STACK`, `0x8075`); buy and sell (`GAME_CMSG_TRANSACT_ITEMS`, `0x804D`); and
`UNEQUIP_ITEM` (`0x804F`), whose 6-byte layout carries no item id and has never been seen from a client.

## 09 - GWPS parity audit

> From `docs/history.md`, moved here on 2026-11-06.


> **History.** The tooling named below was retired when parity closed (docs/16); nothing here is runnable.
> The verdicts are the 2026-10-28 snapshot. Where a row says a behaviour is a stub, missing or shaped one
> way, `docs/history.md` and `MILESTONES.md` hold the answer as the tree stands now; several rows here
> are superseded (`ACCEPT_EULA` writes the version, character creation is delivered, `/tp` is implemented).

The question this answers: **for every behaviour GWPS has, does ASURA have it, and is it the same
behaviour?** Nothing here is a plan to rewrite ASURA like GWPS. The reference is
`C:\Users\Apo\Documents\GitHub\PreSearing` (read-only, never modified); the measure is what GWPS
*observably does*: the messages it puts on the wire, the lines it prints, and the rows it writes.

Citations are `File.c:line` inside `PreSearing\code\OpenTyria` for GWPS and `crate/src/file.rs:line`
for ASURA, so every claim below can be checked rather than believed.

## How to read the verdicts

| Verdict | Meaning |
|---|---|
| **1:1** | Same wire messages and same printed lines for the same input. |
| **1:1 (effect)** | Same observable outcome; the internals differ (e.g. ASURA answers from Postgres where GWPS keeps an in-memory index). Nothing a player or a log reader can tell apart. |
| **AHEAD** | ASURA does something GWPS does not. Always labelled, never called parity. |
| **DEVIATION** | Both answer, differently, and the difference is written down and defended. |
| **BEHIND** | GWPS does something a client can observe that ASURA does not do. |
| **STUB** | The handler exists on both sides and neither does anything a client can see. |
| **DEAD** | GWPS's own handler is unfinished (`TODO`, returns early, sends nothing) - there is nothing to match yet. |

A missing feature is only a gap if GWPS *has* it. Where GWPS is unfinished, ASURA matching GWPS would
mean copying a dead end; those rows are marked **DEAD** and are not work items.

---

## Scoreboard

| Domain | Behaviours audited | 1:1 (incl. effect) | AHEAD | DEVIATION | BEHIND | STUB/DEAD |
|---|---|---|---|---|---|---|
| Auth connection | 15 | 14 | - | - | - | 1 |
| Game connection dispatch | 18 | 12 | - | 2 | 4 | - |
| Movement & agents | 7 | 5 | - | 2 | - | - |
| Chat, commands & system messages | 8 | 7 | - | - | 1 | - |
| Items & inventory | 7 | 3 | 3 | 1 | - | - |
| Character creation | 5 | - | - | - | 5 | - |
| World hosting & map config | 6 | 5 | 1 | - | - | - |
| Persistence & schema | 6 | 3 | 3 | - | - | - |
| Tooling & observability | 6 | 3 | 3 | - | - | - |
| **Total** | **78** | **52** | **10** | **5** | **10** | **1** |

Reading it: **52 of 78 behaviours are indistinguishable from GWPS**, ten are things GWPS cannot do at
all, five differ on purpose and are argued below, ten are behind, and one is a stub on both sides.

The ten BEHIND rows are **two** work items, not ten. Nine of them are character creation: the four
`CHAR_CREATION_*` rows in #2 and the five in #6 describe the same five handlers. The tenth is `/tp`,
which needs one message pair this server cannot yet originate.

Two things this table deliberately does *not* count: GWPS's pathfinding and party modules, which are
library code with no dispatch entry point (see #12), and the five creation rows twice over.

---

## 1. Auth connection

GWPS dispatch: `AuthSrv.c:1728-1784`. ASURA dispatch: `crates/asura-auth/src/lib.rs:348-578`.

| # | Message | GWPS behaviour | ASURA state | Verdict |
|---|---|---|---|---|
| 1 | `SEND_COMPUTER_INFO` | Stores the reported user name and PC name on the connection (`AuthSrv.c:1729-1734`); sends nothing. | Ignored (`lib.rs:352`); the frame is consumed structurally before framing. Nothing observable reads the stored names in GWPS either. | 1:1 (effect) |
| 2 | `SEND_COMPUTER_HASH` | Decodes the hash, then answers `SESSION_INFO` with a salt (`AuthSrv.c:1735-1738`). | Same: random salt, `SESSION_INFO` (`lib.rs:359-373`). | 1:1 |
| 3 | `PORTAL_ACCOUNT_LOGIN` | `AuthSrv_HandlePortalAccountLogin` (`AuthSrv.c:774`): account/password against the DB, then the response the client gates on. | `answer_login` (`lib.rs:375-386`) against the same-shaped schema. | 1:1 (effect) |
| 4 | `ASK_SERVER_RESPONSE` | Answers the request with status 0 (`AuthSrv.c:837-841`), logs the header. | Answers status 0 (`lib.rs:389-403`); the header is only logged under `ASURA_TRACE`. | 1:1 (effect) |
| 5 | `SET_PLAYER_STATUS` | Validates the status and stores it on the connection (`AuthSrv.c:844-864`); nothing reads it and nothing is sent. | No-op (`lib.rs:556`). | 1:1 (effect) |
| 6 | `CHANGE_PLAY_CHARACTER` | Re-reads the character list from the DB, finds the name, remembers the index, answers 0 / network error (`AuthSrv.c:866-897`). | Reads characters from the DB, finds the name, records the selection on the session and in `sessions`, answers 0 / auth error (`lib.rs:414-468`). | 1:1 (effect) |
| 7 | `REQUEST_GAME_INSTANCE` | `AuthSrv_HandleRequestGameInstance` (`AuthSrv.c:899`): picks a live world, sends `TRANSFER_GAME_SERVER_INFO`, then success. | `travel::answer` (`crates/asura-auth/src/travel.rs:96`) probes worlds for reachability, sends the same two messages. | 1:1 |
| 8 | `HEARTBEAT` | Answers a server heartbeat (`AuthSrv.c:1263-1268`). | Answers `messages::heartbeat(16)` (`lib.rs:407-410`). | 1:1 |
| 9 | `DISCONNECT` | Logs and closes the connection (`AuthSrv.c:1758-1761`). | `Ok(false)` - the socket closes (`lib.rs:578`). | 1:1 |
| 10 | `ACCEPT_EULA` | Nothing (`AuthSrv.c:1270-1275`). | Records the accepted version, logs it (`crates/asura-auth/src/lib.rs:587-601`, `account.eula_accepted` from `0012_account_eula.sql`). | 1:1 (effect) |
| 11 | `ADD_ACCESS_KEY` | `AuthSrv_HandleAddAccessKey` (`AuthSrv.c:1277-1290`) logs the key text and keeps nothing. | Counts the key's units and keeps nothing (`lib.rs:485-509`); nothing is logged unless `ASURA_TRACE` is on. | 1:1 (effect) |
| 12 | `DELETE_CHARACTER` | `AuthSrv_HandleDeleteCharacter` (`AuthSrv.c:1292`): deletes when the reasons match, answers per reason. | `db_login::delete_character` with the same reason codes (`lib.rs:511-554`). | 1:1 (effect) |
| 13 | `UPDATE_CHARACTER_SETTINGS` | Answers status 0, stores nothing (`AuthSrv.c:1323-1329`). | Answers status 0 (`lib.rs:470-484`). | 1:1 |
| 14-15 | `SETTING_UPDATE_SIZE`, `SETTING_UPDATE_CONTENT` | `AuthSrv_HandleSettingUpdate` (`AuthSrv.c:656`): logs and answers status 0. | Answer status 0 (`lib.rs:470-484`). | 1:1 (effect) |

**Domain verdict: complete.** All fifteen handlers have a counterpart with the same observable result;
the two effect-level differences (stored computer name, logged access key) are not observable to a
client and are recorded here rather than "fixed" by inventing behaviour.

---

## 2. Game connection dispatch

GWPS dispatch: `GameSrv.c:2007-2074`. ASURA dispatch: `crates/asura-world/src/dispatch.rs:26-101` (22 `GAME_CMSG_` arms).

| # | Message | GWPS behaviour | ASURA state | Verdict |
|---|---|---|---|---|
| 1 | `DISCONNECT` | Closes the connection (`GameSrv.c:1999`). | `Ok(false)` (`lib.rs:1105`). | 1:1 |
| 2 | `HEARTBEAT` | Ignored (`GameSrv.c:2014-2015`). | Ignored (`lib.rs:1194`). | 1:1 |
| 3 | `PING_REPLY` | Ignored (`GameSrv.c:2016-2017`). | Ignored (`lib.rs:1191`). | 1:1 |
| 4 | `PING_REQUEST` | Answers `PING_REPLY` (`GameSrv.c:1978`). | Answers `ping_reply` (`lib.rs:863-866`). | 1:1 |
| 5 | `INSTANCE_LOAD_REQUEST_SPAWN` | The agent's position and the map file id (`GameSrv.c:1566`). | `spawn_block` (`lib.rs:838-847`, block built at `lib.rs:767`). | 1:1 |
| 6 | `INSTANCE_LOAD_REQUEST_PLAYERS` | The player block ending on `INSTANCE_LOAD_FINISH` (`GameSrv.c:1600`). | `player_block` (`lib.rs:851-860`, built at `lib.rs:1602`). | 1:1 |
| 7 | `INSTANCE_LOAD_REQUEST_ITEMS` | Item stream + inventory + weapon slots + gold + faction caps + character info (`GameSrv.c:1660-1690`). | `items_block` (`lib.rs:825-834`, built at `lib.rs:641`). | 1:1 |
| 8 | `CHAR_CREATION_REQUEST_PLAYER` | Warrior default, then a full player-data start/stop block (`GameSrv.c:1691-1719`). | The player-data block with the reference's own creation numbers (energy 20, health 100, property 64 zero) and the attributes (`lib.rs`, `0x8089`) | 1:1 |
| 9 | `CHAR_CREATION_REQUEST_ARMORS` | PvP skills, PvP heroes, PvP items, account features (`GameSrv.c:1721-1739`). | The PvP half as the reference sends it for an account with none: empty skill and hero streams, its PvP item table and the account features (`0x808A`) | 1:1 |
| 10 | `CHAR_CREATION_CHANGE_PROF` | Re-equips the profession's defaults from `GetDefaultEquipments` (`GameSrv.c:1786-1845`, table in `GmDefaultArmors.c`). | The same table, generated from `GmDefaultArmors.c` (`tools/gen-default-equipment.py`: 32 rows, 179 items) | 1:1 |
| 11 | `CHAR_CREATION_CONFIRM` | Creates the character's bags **and writes the item rows once**, then enters the map (`GameSrv.c:1855-1976`; `GameSrv_CreatePlayerBags` at `GameSrv.c:1230`). | The same: the row with the reference's creation defaults, bags and item rows in one transaction, then `CHAR_CREATION_SUCCESS` with its settings block (`0x808B`, `asura_db::inventory::create_character`) | 1:1 |
| 12 | `ITEM_UPDATE_EQUIPPED_COLOR` | Declared as a handler but a no-op: it ignores all three parameters (`GameSrv.c:1847-1853`) | No arm: the fallthrough frames it and ignores it, naming it under trace (`lib.rs:1200-1208`). `docs/history.md` lists it as a missing arm because it counts dispatch entries, not behaviour | 1:1 (effect) |
| 13 | `SEND_CHAT_MESSAGE` | `GameSrv_HandleChatMessage` (`GmChat.c:279-306`). | `0x8064` (`lib.rs:982-1102`). | 1:1 (effect) |
| 14 | `MOVE_TO_COORD` | Pathfinds and broadcasts the first waypoint (`GmAgent.c:371-398`). | The same finder, the same first waypoint, the same refusal (`lib.rs`, `0x803E`; docs #3). | 1:1 |
| 15 | `POSITION_UPDATE` | Adopts / clamps / refuses by a speed budget (`GmAgent.c:409-462`). | `adopt_report` with the same budget (`lib.rs:909-932`). | 1:1 (effect) |
| 16 | `CANCEL_MOVEMENT` | Tick, cancel, broadcast `AGENT_STOP_MOVING` (`GmAgent.c:533-545`). | `agent_stop_moving` (`lib.rs:936-945`). | 1:1 |
| 17 | `LAST_POS_BEFORE_MOVE_CANCELED` | Accepts a short same-plane correction, refuses a long one, sends nothing (`GmAgent.c:547-563`). | `adopt_correction` (`lib.rs:950-972`). | 1:1 (effect) |
| 18 | `DROP_ITEM` | Finds the item, refuses count < quantity with a `TODO`, else creates an item agent and returns (`GmInventory.c:236-300`). | Deducts for real, persists, prints, sends nothing (`lib.rs:1120-1129`, `drop_item` at `lib.rs:1218`). | DEVIATION (deliberate - see #5) |

---

## 3. Movement & agents

GWPS: `GmAgent.c` (564 lines) plus `GmPaths.c` (A*, 1057 lines) and `GmPathHeap.c`.
ASURA: `crates/asura-world/src/lib.rs` arms + `Player::adopt_report`/`adopt_correction`, plus
`crates/asura-world/src/gwdat.rs` + `fa_compress.rs` + `map_data.rs` + `pathfind.rs` for the graph and the
search over it.

The navmesh is no longer the unknown it was: `Gw.dat`'s index, its decompressor, `GmMapDataImport.c` and
`GmPaths.c`'s A* are all ported and measured against the reference's own code (docs/history.md,
item D) - 96 walks, 41 of them found, with every waypoint identical to the last bit except the one case the
reference answers differently on different runs. The finder is also **adopted**: `MOVE_TO_COORD` pathfinds,
answers with the first waypoint, and refuses what it cannot reach by broadcasting where the agent is, while
the world tick walks the agent along the path (`lib.rs`, `agent.rs`). The walk verdict below reflects that.

| Behaviour | GWPS | ASURA | Verdict |
|---|---|---|---|
| Walk to a coordinate | A* over the map's path graph; the answer is the **first waypoint of its own path**, and the refusal is a tick, a cancel and a position broadcast (`GmAgent.c:371-398`) | The same finder (`pathfind.rs`, `GmPaths.c` ported and compared case by case), the same first waypoint; a refused walk is cancelled and **nothing is sent**, measured (`crates/asura-world/src/agent.rs:722-743`) | DELIBERATE DEVIATION, owner ruling open (`crates/asura-world/tests/load.rs:3169`); also the reference divides by zero when a walk's destination is where the agent stands, which poisons its position with NaN, and this keeps the agent where it is |
| World tick | Every `GAME_SRV_MAX_TIME_BETWEEN_WORLD_TICK_MS`, broadcast + advance movement + regen health/energy (`GmAgent.c:502-531`) | `WORLD_SIMULATION_TICK` on the reference's interval; movement advances `speed_base * delta` along the path (`agent.rs`, `advance`) | 1:1 for movement; regen is not integrated - see the health/energy row below |
| Position reports | Adopted within a speed budget, clamped beyond it, refused across a long plane change (`GmAgent.c:409-462`) | Same three outcomes (`lib.rs:920-931`) | 1:1 (effect) |
| Cancel a walk | `AGENT_STOP_MOVING` after a tick (`GmAgent.c:533-545`) | `AGENT_STOP_MOVING` (`lib.rs:936-945`) | 1:1 |
| Correct after a cancel | Short correction on the same plane taken, long refused, nothing sent (`GmAgent.c:547-563`) | Same (`lib.rs:950-972`) | 1:1 (effect) |
| `/stuck` | `WorldTick` + cancel + broadcast position (`GmChat.c:243-251`) | `AGENT_UPDATE_POSITION` (`lib.rs:1038-1062`); the 500 ms ticker covers the tick | 1:1 |
| Agent health/energy regen | Server-side integration during the tick (`GmAgent.c:524-529`) | Fixed values sent once in `player_block`; no regen integration | DEVIATION - invisible with one player and no combat; recorded, not fixed |

---

## 4. Chat, commands & system messages

GWPS: `GmChat.c` (306 lines), `GmCommands.c` (259 lines), `GmText.c`.
ASURA: `lib.rs:974-1102`, `maps_answer` at `lib.rs:1497`.

The single most important finding of this audit: **GWPS puts no chat text on the wire for anything the
*server* generates.** Every server-side answer funnels through `GameSrv_SendSystemChatMessage`
(`GmChat.c:195-233`) or `GameSrv_SendInvalidCommand` (`GmChat.c:145-180`), and both return early at
`GmChat.c:209-212` / `GmChat.c:150-153` unless `GWPS_CHAT=1`. The reference's own comment
(`GmChat.c:129-138`) says why: sending chat crashed its client twice, and its layouts are still
unsettled. A player's `!`/`#`/`$` line *is* sent (`GmChat.c:259-277`, not gated), and the user has
never seen one appear - in either server.

| Behaviour | GWPS | ASURA | Verdict |
|---|---|---|---|
| Player speaks on `!` | Ungated broadcast of the text + `CHAT_MESSAGE_LOCAL` on `Channel_All` (`GmChat.c:259-277`, `290`) | `chat_message_core` + `chat_message_local(PLAYER_ID, CHANNEL_ALL)` (`lib.rs:1010-1030`) | 1:1 (effect) - bytes on the wire in both, visible in neither |
| Player speaks on `#` / `$` | Same path with `Channel_Group` / `Channel_Trade` (`GmChat.c:293-294`) | Same (`lib.rs:1011-1015`) | 1:1 (effect) |
| Player speaks on `@`, `%`, `"` | Silently ignored (`GmChat.c:291-298`) | Ignored (`lib.rs:1086-1095`) | 1:1 |
| Player speaks on any other channel | Error log, `ERR_BAD_USER_DATA` (`GmChat.c:300-302`) | Prints a refusal line (`lib.rs:1096-1099`) | 1:1 (effect) |
| `/maps` | Chat aliases via the gated sender **plus** an unconditional `log_info` table: `Teleport targets (N aliases, every one with a map config):` then `  /tp %-15s -> map %3d  %s` (`GmCommands.c:113-152`) | The same table, printed, character for character (`maps_answer`, `lib.rs:1497-1529`); the chat half is printed only under trace and never sent (`lib.rs:1063-1078`) | 1:1 |
| Unknown `/command` | "Unknown command" through the gated sender => log line only (`GmChat.c:145-180`) | `world: Unknown command` (`lib.rs:1082`) | 1:1 |
| `/stuck` | Cancel + position broadcast (`GmChat.c:243-251`) | `AGENT_UPDATE_POSITION` (`lib.rs:1038-1062`) | 1:1 |
| `/tp`, `/teleport` | `GameSrv_HandleTeleportCommand` (`GmCommands.c:155-216`): refuses a missing or unknown alias, refuses a map with no config (it would silently fall back to Kamadan, `GmCommands.c:180-190`), refuses "already here", then `GameSrvCtrl_StartPlayerTransferTo` starts a real server transfer. Both refusals go through the gated chat sender, so with the gate off they are log lines. | The same four outcomes, in the same order, with the same lines (`teleport`, `lib.rs`): unknown alias, no config, already here, then a `transfer` row and the reference's redirect. The alias table is the reference's own, generated (`travel_targets.rs`) | 1:1 |
| The alias table `/tp` and `/maps` read | `s_teleport_targets` (`GmCommands.c:32-53`): twenty aliases over twelve maps, two for most (`lakeside` and `lakeside county`) | The same twenty, generated from that table by `tools/gen-map-config.py` and compared row for row by `tools/parity-commands.py` (`docs/history.md`). Until it was generated, `/tp` matched the map's **name**, so six of the twenty worked | 1:1 |

### What a teleport is, measured

`/tp` is not a position change; it is a server transfer, and the reference has it working. The wire
evidence, none of which is a guess:

- **`TRANSFER_GAME_SERVER_INFO` (`0x01A6`)**, 46 bytes as GWPS sends it and 52 in the live capture (the
  extra 4 are the client's own 4-byte game header): `{header, host[24], map_token, region, map_id,
  is_explorable, player_token}`. The 24-byte host is a `sockaddr_in`: family (little endian), port (big
  endian), four address bytes, then padding. The two captured packets carry `map_id = 161,
  is_explorable = 1` and `map_id = 165, is_explorable = 0` - which is how the rule "the flag describes
  the *destination*, not the source" was found (`GameSrv.c:1394-1403`).
- **`MAP_UPDATE_CURRENT` (`0x0099`, 12 bytes)** follows in the same millisecond, in 10 of 10 captured
  transitions (`GameSrv.c:1429-1432`).
- **A real client-announced travel begins with `TRAVEL_INIT` (`0x80C1`, 10 bytes on the wire,
  client -> server, two agent ids)** - the message `docs/03-protocol.md` 5f measures; GWPS's `/tp` skips
  it, that is its documented race: the redirect arrives in a map the client never asked to leave, and
  under latency the hop can fail (`docs/08-gm-commands.md`).

Sources: `captures/zone-ref-005829.jsonl` seq 42 and 424 (raw, gitignored), `docs/08-gm-commands.md`
("What a teleport actually does", "Known limitation"), `code/OpenTyria/GameSrv.c:1360-1435`,
`opcodes.h:358`. ASURA already carries the builder and the sizes
(`crates/asura-protocol/src/game/mod.rs:59-61`) and the whole cycle is written down in
`docs/03-protocol.md` #5 - what the world lacks is the ability to *start* one.

---

## 5. Items & inventory

GWPS: `GmInventory.c` (drop handler), `GameSrv_CreatePlayerBags` (`GameSrv.c:1230`), `GmItem`.
ASURA: `crates/asura-db/src/inventory.rs` (rows + moves), `crates/asura-world/src/item.rs` (blocks),
`crates/asura-protocol/src/game/mod.rs` (builders).

| Behaviour | GWPS | ASURA | Verdict |
|---|---|---|---|
| Where items live | In-memory `GmBag`/`GmItem` arrays on the player, written to the DB once at character creation (`GameSrv.c:1230`, called only from `HandleCharCreationConfirm`) | Rows in `bag`/`item` (`crates/asura-db/migrations/0006_inventory.sql`), read at login (`inventory.rs:314`) | AHEAD - ASURA's items survive a restart; GWPS's do not |
| The item block on entering a map | Item stream, one `CREATE_NAMED_ITEM` per item with a **`u8`** modifier count, bags in the reference's own fixed order | Byte-identical block, proven against the capture fixture (`crates/asura-world/src/lib.rs:631` `the_item_block_opens_with_the_stream_and_carries_the_capture`) | 1:1 |
| Move an item to a slot | **No handler.** `GAME_CMSG_ITEM_MOVE` is not in GWPS's dispatch | `0x8072` -> `move_item` -> echo `ITEM_MOVE_TO_LOCATION`, persisted (`item.rs:256` `move_item`, `item.rs:498` `handle_item_move`, `dispatch.rs:75`, persisted in `inventory.rs`) | AHEAD |
| Equip an item | **No handler.** No unequip, trade, split, salvage, identify or use handler either | `0x8030` -> equipped bag + `UPDATE_AGENT_VISUAL_EQUIPMENT` (`item.rs:315` `equip_item`, `item.rs:535` `handle_equip_item`, `dispatch.rs:77`, `equipped_slot` at `mod.rs:200`) | AHEAD |
| Drop an item | Refuses `count < quantity` (`TODO`), never removes the row, creates an agent whose model id is the *item* id (`GmInventory.c:236-300`) | Deducts, persists, prints; sends nothing because both candidate messages crashed a live client and neither is in a capture (`item.rs:187` `drop_item`, `item.rs:477` `handle_drop_item`, `dispatch.rs:73`; `drop_messages` at `item.rs:251` is the shape the crash ruled out) | DEVIATION - deliberately not the reference's dead end |
| Unequip | **No handler.** | `0x804F` logged as unimplemented - its layout carries no item id (`item.rs:555` `handle_unequip_item`, `dispatch.rs:79`) | 1:1 (both do nothing, ASURA says so) |
| Item colour update | Declared, ignores all three parameters (`GameSrv.c:1847-1853`) | No arm; the fallthrough frames it and ignores it (`dispatch.rs:26-101`, the arm that names no handler) | 1:1 (effect) - neither does anything |

**The honest summary of this domain:** GWPS's item support is a load request and a broken drop. Three of
ASURA's item behaviours have no GWPS counterpart at all, so "parity" here means ASURA is ahead and the
gap list has one real entry: the **ground-item message** ASURA cannot send yet because no capture
contains it.

---

## 6. Character creation

GWPS: `GameSrv.c:1691-1976`, `GetBagSlotForItemType` (`GameSrv.c:1741-1784`), `GmDefaultArmors.c`.

| Behaviour | GWPS | ASURA | Verdict |
|---|---|---|---|
| Creation instance | `MapType_CharacterCreation` (11), its own map type in `MapType` | Hosted: the world serves type 11 (`MAP_TYPE_CHARACTER_CREATION`, `main.rs`) and a fresh account reaches the creation screen (`a_character_is_created_in_a_creation_instance`) | 1:1 |
| `CHAR_CREATION_REQUEST_PLAYER` | Warrior by default, then a full player-data block | The player-data block, with the creation path's own three int properties (`0x8089`) | 1:1 |
| `CHAR_CREATION_REQUEST_ARMORS` | PvP skills/heroes/items, account features | The same four blocks (`0x808A`) | 1:1 |
| `CHAR_CREATION_CHANGE_PROF` | Re-equips from a 2858-line generated table | The same table, generated (`default_equipment.rs`, 32 rows / 179 items) | 1:1 |
| `CHAR_CREATION_CONFIRM` | Writes the item rows once, then enters the map | The same, in one transaction, with the reference's creation defaults (`0x808B`) | 1:1 |

This used to be the largest genuine gap - one *feature*, "new characters can be created", wearing five
dispatch names. It closed when the creation instance, the four handlers and the generated armour table
landed; `docs/history.md` records T6 as measured, and the creation test asserts the three int
properties the reference's creation path sends rather than the load path's numbers.

---

**The catalogue is data, and it is now generated.** The reference's three tables describe twelve maps a
client can be sent to: `GmCommands.c:32-53` (every map with a config, its aliases and its **`MapType`**),
`GmMaps.h` (the numbers) and `GmMapsConfig.c` (file id and spawn point). `tools/gen-map-config.py` reads all
three and writes `tools/map-config.sql`, which `tools/seed-map.ps1` applies; the battery re-generates with
`--check`. Before that, this repo's seed carried two hand-typed maps and called Lakeside County (146) an
**outpost** - the reference says `MapType_MainExplorable`, and its roadmap records that map as *verified*
loading.

That correction exposed a second liberty: this world refused to host explorables at all, on the theory that
it "cannot spawn anything into one". A world server does not spawn content - it hands the client the blob and
the spawn point, which is what the reference does with 146. Explorable hosting is allowed now (types 0, 1 and
11), and a world hosts **every map in the catalogue** by default: the reference pre-spawns one server per
configured map (`AuthSrv_PreSpawnMapServers`), so `/maps` lists all twelve and `/tp` reaches all twelve
without a flag. `--maps` narrows the set on purpose - a soak, a single-map test - and is never required.

That default was wrong for a long time in a way nobody had written down: `asura-world` without `--maps`
hosted **one** map (146, the constant from the first single-map server), so `/maps` printed one alias and
`/tp` to any other map was refused. The reference has no such state. The regression test
`the_default_map_list_is_the_whole_catalogue_and_tp_reaches_it` holds that line now.

## 7. World hosting & map config

GWPS: `GmMapsConfig.c` (252 lines) + pre-spawned map servers + `CtrlConn` (`GameSrv.c:48-75`,
`1327-1389`). ASURA: `crates/asura-world/src/main.rs`, `crates/asura-db/src/game.rs`.

| Behaviour | GWPS | ASURA | Verdict |
|---|---|---|---|
| Where a map's facts come from | `GmMapsConfig.c`: file id, type, spawn point, spawn plane, capacity, aliases | `map_config` rows + migration 0007's `name` column | 1:1 (effect) |
| Aliases for `/tp` and `/maps` | Config string, spaces collapsed | The reference's own table, generated (`travel_targets.rs`); derived from the map name only for a map the table does not know | 1:1 |
| Refusing a map a client cannot load | `map_file_id == 0` refused | Refused with a reason (`main.rs:161-176`) | 1:1 |
| Refusing explorable areas as hosts | Type check | Same check (`main.rs:170`) | 1:1 |
| Advertising a world | `CtrlConn` message to the auth service; heartbeat keeps it alive | `register_world`, heartbeat task (`main.rs:206-240`) | 1:1 (effect) |
| Starting the map server for a travel | Auth service spawns the map server on demand | Auth service registers/chooses a world; `start-server.ps1` starts the process | AHEAD (process control by script vs by the auth service) |

---

## 8. Persistence & schema

GWPS schema (read at `Db.c:269-360`): sessions, accounts, characters, bags, items, friendships; the
binaries check a schema version. ASURA: migrations 0001-0007 - `schema_version`, `account`, `session`,
`world`, `world_pool`, `characters`, `map_config`, `lease`, `transfer`, `bag`, `item`.

| Table / behaviour | GWPS | ASURA | Verdict |
|---|---|---|---|
| accounts / sessions | Present, same shape (`Db.c:278-287`) | `account`, `session` (`0001`) | 1:1 |
| characters | Present (`Db.c:296`) | Present + `pos_plane` for saved positions (`0006`) | AHEAD (positions persist) |
| bags / items | Present (`Db.c:325`, `Db.c:355`); rows written once at character creation | `bag`, `item`; read per map entry, moved and persisted per action | AHEAD |
| friendships | Table read, but the login response only sends the stream **terminator** (`AuthSrv.c:831`); no entry is ever sent | No table; `friend_stream_end` is sent at the same point in the login reply (`crates/asura-auth/src/lib.rs:698`) | 1:1 (effect) |
| Migrations | A schema version the binaries check | `schema_version` row; the world refuses to start against a stale schema (`main.rs:142-147`) | 1:1 (effect) |
| SQL ownership | Queries scattered through the C code | Only `asura-db` writes SQL (`docs/08`) | AHEAD |

---

## 9. State parity: what each server keeps for a connected player

**This section exists because the first version of this audit did not have it, and that omission hid a
real liberty.** The rest of the document compares dispatch arms and wire messages - what goes in and what
comes out. It never compared **what the two servers hold between messages**, and that is where a missing
capability hides: a server that keeps less cannot do what the other one does, even when every message it
*can* answer is identical.

GWPS keeps twelve fields per player (`GmPlayer.h:21-33`). ASURA keeps the same twelve as the `Player` struct in `crates/asura-world/src/player.rs:18-50`, with the ids allocated per connection (`instance.rs:70`) rather than fixed constants.

| kept | GWPS | ASURA | Verdict |
|---|---|---|---|
| player id | `GmPlayer.player_id` | `Player.player_id`, allocated per join (`instance.rs:70`, `:291`) | 1:1 |
| agent id | `GmPlayer.agent_id` | `Player.agent_id`, allocated with the player id | 1:1 |
| connection | `GmPlayer.conn_token` | the connection is the task; no id | 1:1 (effect) |
| **character id** | `GmPlayer.char_id` | `Player.character` | 1:1 |
| **account id** | `GmPlayer.account_id` - used 63 times on the game side, including every bag and item read (`GameSrv.c:1136`, `GameSrv.c:1174`) and the transfer request (`GameSrv.c:1456`) | `Player.account_id`, kept from the claimed transfer; every bag and item query and the `/tp` transfer name it | 1:1 - it was the one real liberty in this table until `/tp` needed it |
| character row | `GmPlayer.character` (a whole `DbCharacter`: ~44 columns) | the columns a message reads - `appearance`, `primary_profession`, `unlocked_professions`, `gold` (migration 0008) and `level`, `campaign`, `is_pvp`, `secondary_profession`, `helm_status` (migration 0009), plus the position this schema keeps for itself - and the settings blob built from them | **1:1 for what is sent, and it used to be worse**: until migration 0008 the load block sent the reference's *default* character to everyone, and until migration 0009 the login reply's character list sent **one captured 62-byte blob for every character** (a Warrior, level 1, outpost 146, five dyed armour pieces) instead of each character's own |
| account row | `GmPlayer.account` (a whole `DbAccount`) | nothing; read per request | 1:1 (effect) |
| bags | `GmPlayer.bags` (in memory, written once) | `Player.inventory` (rows, reloaded) | 1:1 (effect) |
| campaign | `GmPlayer.char_creation_campaign` | `characters.campaign` (migration 0009), written by the creation confirm and carried in the settings blob | 1:1 |
| profession | `GmPlayer.primary_profession` | `characters.primary_profession`: `PLAYER_UPDATE_PROFESSION` sends the row's, and the creation screen's choice is what the row holds | 1:1 |
| team token | `GmPlayer.player_team_token` (an arbitrary constant) | `PLAYER_TEAM_TOKEN` | 1:1 (effect) |
| party id | `GmPlayer.party_id` | `PARTY_ID` (1) | 1:1 (effect) |

Two conclusions, and they are the answer to "why does this project take liberties":

1. **Almost every difference is storage, not behaviour** - ASURA derives from rows what GWPS caches. That is
   legitimate, but only because it *is* recorded here: the rows are the same rows, read from the same place,
   and nothing a client can see differs. Stated as a rule: **caching nothing is fine; being unable to answer
   is not.**
2. **The one liberty this table used to have - `account_id` - is gone.** GWPS keeps it because work needs it:
   every bag and item query is scoped by it, and the transfer request carries it
   (`GameSrv.c:1450-1472`: `player_id`, `account_id`, `char_id`, `district`). This server dropped it until
   `/tp` needed exactly that, which is when it was kept; the rows above are the record of it.

**What is still not stored, and why that is written down rather than fixed.** The reference's
`DbCharacter` carries about forty-four columns. This server stores the one every message that exists reads:
`appearance`, `primary_profession`, `unlocked_professions`, `gold` (migration 0008) and `level`, `campaign`,
`is_pvp`, `secondary_profession`, `helm_status` (migration 0009), and the armour pieces the settings blob
carries are *derived* from the equipped bag rather than denormalised. The rest - skills, the 1024-bit
unlocked-skill bitmap, the four 256-bit completion bitmaps, skill points, experience - stay out because
nothing writes them (GWPS's own confirm sets them to zero and no handler ever assigns one) so the zeros are
parity; argument and evidence in `docs/history.md`, item B5. A column nothing loads is the same
liberty in a new place, which is the rule this section exists to enforce.

ASURA additionally needs its own `session_id` on the connection. That is **not** parity with `GmPlayer` -
GWPS keeps no session id on the game side; its analogue lives auth-side, on the auth connection
(`AuthConnection.account_id`, `account.current_char_id`), which is what mints the token. ASURA's `transfer`
and `lease` rows are keyed by session, so the session is the thing it must carry to mint one itself.

---

## 10. Tooling & observability

| Behaviour | GWPS | ASURA | Verdict |
|---|---|---|---|
| Packet trace | `GWPS_TRACE=1` | `ASURA_TRACE=1` (`tools/play.ps1 -Trace`) | 1:1 |
| Event log that is not a packet log | `log_info` lines for dispatch and GM answers | Unconditional `eprintln!` event lines (`docs/03` #11) | 1:1 |
| Captures in the repository | `GWPS_TRACE=1` output kept as text | `crates/asura-db/src/inventory.rs:243` (`default_inventory()`) is a capture turned into a fixture | AHEAD (a capture that fails the build when it drifts) |
| Web login gate | `tools/webgate.py` on 6601 | The same script, started by `tools/start-server.ps1` | 1:1 |
| One-command server | Several consoles | `tools/start-server.ps1` / `stop-server.ps1` / `play.ps1` | AHEAD |
| Test battery | None in the reference | `tools/test-battery.ps1` + the workspace suite (`cargo test`) | AHEAD |

---

## 11. ASURA behaviour GWPS does not have (explicitly AHEAD)

Recorded here so nobody later "restores parity" by deleting working features:

1. Items survive a restart, and a drop deducted from a stack is gone after a reconnect.
2. Item moves and equips are handled, persisted and echoed.
3. Character positions persist across sessions (`pos_plane IS NULL` means never saved).
4. `map_config.name`, and a `/maps` table that names the map as well as aliasing it.
5. Integration tests that assert the wire (`the_item_block_from_rows_is_the_capture_byte_for_byte`,
   `maps_prints_its_table_and_puts_nothing_on_the_wire`).
6. The world refuses to start against a stale schema instead of failing later.

---

## 12. Where GWPS is itself unfinished (the "missing" that is not a gap)

These are GWPS rows that look like features and are not. Matching them would be copying a dead end:

1. **Drop with `count < quantity`** - `log_warn("TODO: we must still implement this")`
   (`GmInventory.c:283-288`). The client always sends `count = 1` for a stack, so dropping a stack does
   nothing at all in GWPS.
2. **The item agent** - `item_agent->model_id = item->item_id` (`GmInventory.c:300`), so the thing on
   the ground is drawn as nothing; and the item is never removed from the bag.
3. **All server chat** - gated off by default because it crashed the client twice (`GmChat.c:129-143`).
4. **`GmPaths.c` / `GmPathHeap.c`** - A* with no dispatch entry point; `GameSrv_HandleMoveToCoord` is its
   only caller. (Both are ported here regardless: the refusal is a behaviour a player notices, so the queue
   adopts them - see item D.)
5. **`GmParty.c`** (120 lines) - `GameSrv_`-prefixed writers with no `GAME_CMSG_PARTY_*` handler.
6. **`GmDefaultArmors.c`** - a table with one caller, `CharCreationChangeProf`.

---

## 13. The real gap list

Ordered by what a player or tester would notice first. Each entry says what it needs, and nothing else
is added to it. **Everything that used to be on this list and is not here now has a measurement behind it** -
dispatch, the emit surface, the load block, movement and pathfinding, `/tp`, character creation, multi-player,
the schema and the command surface - and the generated pages are where to check rather than this paragraph:
`docs/history.md`, `docs/history.md`, `docs/history.md`, `docs/history.md`.

| # | Gap | What it needs | Blocked on |
|---|---|---|---|
| 1 | The ground item for a drop | The message that spawns a dropped item on the ground | **A measurement that does not exist**: a capture of dropping a quantity-1 item. The reference's own handler is the unfinished one (a `TODO` for partial stacks, the item never removed, its model id the item's id), so it cannot produce one |
| 2 | Unequip (`0x804F`) | The item id the client means | **A measurement that does not exist**: a capture. The message's layout carries no item id, and dragging an item out of the equipped bag arrives as `ITEM_MOVE`, which works |
| 3 | The live halves of T9, T10, T11 and T13 | A second real client rendering the other player, the walk and the chat, and a human watching memory over ten minutes | **Not a measurement gap**: it is a rig run. The server halves are asserted (a second agent introduced and withdrawn, the reference's own path and refusals, the chat pair, the tick and ping rhythm) and `docs/history.md` records each as `hand` |

Two things are worth stating plainly because they look like gaps and are not. **Chat rendering (T11)** is not
achievable by the reference either: its own chat send path is gated off because it crashed its client twice, so
"both clients display it" is a criterion GWPS cannot meet. This server matches what GWPS *does* - sends a
spoken line, prints a generated answer - and that is the ceiling. And **walking** is no longer a gap: the path
finder is ported and compared case by case, and `MOVE_TO_COORD` answers with the reference's own waypoint.

---

## 14. How to check this audit

Most of this document is now re-derived by tools, and the tools win where they disagree with it:

```powershell
# every axis, re-measured, in one page
python tools/parity-summary.py --write docs/history.md --fast
# dispatch and the emit surface
python tools/parity-report.py --write docs/history.md
python tools/parity-emit.py
# the database schema, column by column
python tools/parity-schema.py --check
# the /tp aliases and the whole command surface
python tools/parity-commands.py --check
# the reference's own test plan, item by item
python tools/parity-testplan.py --check
# the load block: order and size, then the bytes
python tools/parity-load.py --check crates/asura-world/src/load_fixture.rs
python tools/parity-load-bytes.py --check
# the map data and the path finder, against the reference's own code
python tools/parity-pathing.py --check
python tools/parity-path.py --check
# everything at once, which is what a change has to keep green
.\tools\test-battery.ps1
```

The hand checks that remain -- the ones the tools cannot make -- are listed in `docs/history.md` as
`hand`, with what each needs.

## 10 - The parity work queue

> From `docs/history.md`, moved here on 2026-11-06.


> **History.** The tooling named below was retired when parity closed (docs/16); nothing here is runnable.

The whole surface, measured, in one place. `docs/history.md` answers "is behaviour X
equivalent?"; this document answers "**what still has to be built, in what order, and why**".

Two rules, both learned the hard way in this repo:

1. **A gap enters this queue only with evidence** - a capture, a dispatch table, a generated diff, or
   a line of the reference's own source. "I read it and it looked finished" is how three wrong claims
   got into `docs/09` (`/tp` "in no capture", state parity, the party messages).
2. **The reference's own tests are the acceptance criteria.** GWPS publishes T0-T15 in
   `PreSearing/docs/04-scope.md`. Where a test exists, closing a queue item means passing that test,
   not "feels done".

## The yardstick: GWPS's own test plan

The reference's sixteen items (`PreSearing/docs/04-scope.md`) are the closest thing either project has to a
definition of the observable surface, and the mapping from them to this repo's checks is now **generated**:
`docs/history.md`, by `tools/parity-testplan.py`. It parses the items and their pass criteria out of the
reference's document, names the check that covers each one, and verifies that every named check still
exists -- a renamed test breaks the check rather than quietly rotting the page. Today: **12 measured, 3 hand,
1 blocked**.

What is left, in that page's own words: **T12** (the ground item) is blocked on a capture of a working drop,
which the reference cannot produce because its own handler is the unfinished one; **T9, T10 and T11** are
`hand` -- the server halves are asserted (a second agent, the reference's own path and refusals, the chat
pair) and the *seeing* of them needs a second real client. The table this section used to carry by hand went
stale three times; the generated page is what replaced it.

## The four axes, as measured today

| axis | how it is measured | current state |
|---|---|---|
| client -> server dispatch | `tools/parity-report.py` vs `GameSrv.c`/`AuthSrv.c` switches | **complete**: auth **15/15**, game **18/18**; the battery reports "nothing missing" |
| server -> client emit | `tools/parity-emit.py` | **complete for every message the reference sends**: 76 of 81 reachable, **0 real gaps**, 5 explained absences |
| state kept per player | `docs/09` #9, hand-checked against `GmPlayer.h:21-33` and `DbCharacterColumnsDef` | the four traits a message reads are stored (migration 0008) and the block sends them instead of constants; the rest of the reference's ~44 character columns are queue item **B5**, listed with the reason |
| schema | migrations vs `Db.c` queries | equivalent for what is used; no `friendships` table (the stream is empty anyway) |
| the map catalogue | `tools/gen-map-config.py` against the reference's three tables | **12 maps, the reference's own list**: aliases and `MapType` from its teleport table (`GmCommands.c:32-53`), numbers from `GmMaps.h`, file ids and spawns from `GmMapsConfig.c`. The seed used to carry two hand-typed maps |
| the load block | `tools/parity-load.py` against the reference's own `GWPS_TRACE=1` log, as a generated fixture (`crates/asura-world/src/load_fixture.rs`) | **measured: 480 messages, 0 missing, 0 extra, 0 size disagreements** - it matches message for message |

The emit tool counts a message as emitted only when a **send path** reaches the builder: a builder
nothing calls is a gap, and so is one only a unit test calls. That rule is why the four party/leave
builders (A4) still appear in the gap list - they exist, and nothing sends them yet.

## The queue

Ordered by dependency, not by size. Every row cites its evidence.

### A. Multi-player maps - the reason half the missing messages were missing

GWPS handles **N players per map**: it broadcasts a player's arrival to everyone, tells everyone when
a player leaves, and keeps a party per player. ASURA used to be single-player **by construction** -
`PLAYER_ID` and `PARTY_ID` constants, one agent per connection, `Agents` with one slot - and that is why
`PLAYER_DESTROY`, `WORLD_REMOVE_AGENT`, `PLAYER_UPDATE_PARTY`, `PLAYER_UPDATE_PARTY_SIZE` and the
`player_block`'s pair were missing (T9, T12 consistency, T14). Evidence for the old state:
`tools/parity-emit.py` output; `GmPlayer.c:314`, `GmAgent.c:60`,
`GmPlayer.c:322`, `GmPlayer.c:346`; capture `zone-ref-005829.jsonl` seq 254-259.

**A1-A5 landed.** Each connection now allocates its own player and agent id instead of using the
constants, a `Peers` registry holds everyone in the map with a channel per connection, a joining player
is introduced to the others and they to it in `GameSrv_SendWorldAgents`'s order, a leaving one is
withdrawn with the farewell pair, moves and position reports are broadcast, and the test
`a_second_player_is_introduced_to_the_first_and_withdrawn_again` covers all of it. `PLAYER_ID`/`AGENT_ID`
remain only as the ids the first player gets.

The four messages it needed turned out to be wrong in the **generated table**, not just missing: the
client's own table says 5, 6, 6 and 6 bytes for them while every live capture says 12, 12, 8 and 8,
because they carry a four-byte game header (`TYPE_MSG_HEADER_GAME`, `msgdefs.h:19-23`). The generator
now applies that per message, with the captures named in `OVERRIDES`.

| # | item | state |
|---|---|---|
| A1 | an agent table per map instead of `PLAYER_ID`; each connection owns its agent | done |
| A2 | a joining player is broadcast (`PLAYER_CREATE`, `WORLD_CREATE_AGENT`) to the others | done |
| A3 | a leaving player is withdrawn (`PLAYER_DESTROY`, `WORLD_REMOVE_AGENT`) | done |
| A4 | per-player party messages (`PLAYER_UPDATE_PARTY_SIZE`, `PLAYER_UPDATE_PARTY`) | done |
| A5 | movement of one player is broadcast to the rest | done |

### B. Character creation (T6)

Four handlers, and the instance they arrive in (`MapType_CharacterCreation`, 11). Evidence:
`GameSrv.c:1691-1976`, `GetBagSlotForItemType` (`GameSrv.c:1741`), `GmDefaultArmors.c` (2858 lines of
data), `CHAR_CREATION_START/SUCCESS/ERROR` in the emit diff.

| # | item | done when |
|---|---|---|
| B1 | host the character-creation instance (`MapType_CharacterCreation`) | a fresh account reaches the creation screen |
| B2 | `CHAR_CREATION_REQUEST_PLAYER` + `_REQUEST_ARMORS` (the player-data and PvP blocks) | no `Unhandled GameSrv packet` in the log |
| B3 | `_CHANGE_PROF` with the default-equipment table ported from `GmDefaultArmors.c` | picking a profession swaps the equipped items |
| B4 | `_CONFIRM`: write `characters`, `bags`, `items` rows | the same three `count(*)` that T6 prints |

**B landed, measured.** The *data* is ported and generated: `tools/gen-default-equipment.py`
reads `GmDefaultArmors.c` and writes `crates/asura-world/src/default_equipment.rs` - **32 (campaign,
profession) rows, 179 items**, the field set being the reference's own `GmItem` literal, which is also what
`CREATE_NAMED_ITEM` writes. `every_profession_starts_with_the_references_own_equipment` pins a Prophecies
Warrior's chest piece to the reference's bytes, and the battery re-generates with `--check`.

The *messages* are built too: `char_creation_start` (2 bytes), `char_creation_error` (6, header plus the
reference's `GM_ERROR_CHARACTER_NAME_ALREADY_EXIST` slot), `char_creation_success` (uuid + counted name +
the reference's own `0x2211` + counted settings) and `instance_player_data_done` (2), all sized by the
table and asserted in `the_fixed_size_messages_match_the_size_the_reference_sends`.

**B1-B4 landed.** The data is generated, the messages are built, and the flow itself is in: the world hosts
map type 11, all four handlers answer (`0x8089`, `0x808A`, `0x808B` and the profession change), and the
confirm writes the character's row, bags and item rows in one transaction before sending
`CHAR_CREATION_SUCCESS` with the settings block. What is *not* verified is the screen itself - a live client
walking the creation UI - which is why `docs/history.md` records T6 as measured with the layout as the
rig's job.

### B5. The rest of the character row (~40 columns)

The reference's `DbCharacter` (`DbSchema.h:66-110`) stores about forty-four columns per character. This
server stored the ones a message read *when the column was added* - appearance, primary profession, unlocked
professions, gold (migration `0008_character_traits.sql`) - and now also the five the **character settings
blob** needs: `level`, `campaign`, `is_pvp`, `secondary_profession`, `helm_status` (migration
`0009_character_settings.sql`).

**What B5 actually was: one captured blob sent for everybody.** `AUTH_SMSG_CHARACTER_INFO` carries a 62-byte
`CharacterSettings` block per character - the appearance, level, campaign, professions, helmet status, last
outpost and five armour pieces with their dyes, which is what the character-select screen draws. The auth
service was sending a constant copied from a live reference reply: *the reference's seeded Warrior*, level 1,
last seen in outpost 146, wearing five pieces dyed the same colour. Every character in the list looked like
that one. Now the block is built from the row (`messages::character_settings`, `GmChar.c`'s packing), and the
armour pieces come from the equipped bag instead of the reference's denormalised `file_id_*`/`colors_*`
columns - same answer, one copy of it.

The measurement that the packing is right: `the_settings_block_built_from_a_row_is_the_references_own_for_that_character`
builds the block from that captured Warrior's fields and requires it to equal the reference's own bytes, all
62 of them. The auth test then requires the *seeded* character's block to differ from that constant and to
carry its own row's values, and the database test that `create_character` writes what `settings` reads back.

**Most of the remaining columns are still *not* work, and that was measured rather than assumed.** Checking
them against what the reference actually does split the list in two:

| column(s) | the reference's own behaviour | verdict |
|---|---|---|
| `skill1`..`skill8` | `GameSrv_SendSkillbarUpdate` sends them from the row, and `HandleCharCreationConfirm` builds that row with `= {0}` and never assigns a skill - nothing in GWPS writes one | zeros are parity |
| `unlocked_skills` | `GameSrv_SendUpdateUnlockedSkills` sends the bitmap, which is written as all-zero by the same confirm and assigned nowhere | an empty list is parity |
| skill/attribute points | `GameSrv_SendAgentAttributePoints` **hardcodes** unused 50 / used 50 | the constant is parity |
| the attribute spread | `GameSrv_SendPlayerAttributes` sends `data_len = Attribute_Count` and no values at all | the zeros are parity |
| `unlocked_maps`, the four completion bitmaps | stored, never read into a message that exists | no observable difference |
| `experience`, `skill_points`, `skill_points_total` | read by `GameSrv_SendPlayerFactions` (`0x00E9`) and written by nothing: no handler awards experience | the zeros this server sends are parity |
| faction points (twelve `balthazar`/`kurzick`/`luxon`/`imperial` columns) | read into `0x00E9` and the four `FACTION_MAX_*` messages | this server sends the fresh-account defaults (max 10000, everything else zero), which is what the reference's own columns hold for an account that has done nothing |
| `last_guild_hall_id` | read into the settings blob | zero: nothing here creates guilds, and the reference's own value for a lone character is the null uuid |
| `storage_gold` | `GameSrv.c:825` puts it in the storage-gold message | not sent: this server sends the character's gold, and an account-level storage pane needs storage bags that the reference never fills either |
| `eula_accepted` | read from the row, but `AuthSrv.c:684` sends a **hardcoded 24** to the client | unobservable: the value on the wire is a constant in the reference too |

One thing that *was* wrong surfaced through the same reading, and is fixed: the character-creation block sent
the **load** path's health and energy (25, 400 and a regen float) where the reference's creation path sends
its own three int properties - energy 20, health 100, property 64 zero (`GameSrv_SendPlayerHealthEnergy`,
`GmPlayer.c:178-199`). Asserted: the creation test checks those three values and the absence of a float
property.

**The schema axis is now generated rather than argued.** `tools/parity-schema.py` parses `DbSchema.h` into
its five tables and 95 columns and compares them with this database, printing `docs/history.md` and
failing on any reference column that is neither stored, nor derived, nor explained. The numbers today:
**22 stored, 11 mapped under a different name, 7 packed into `appearance`, 55 explained**. That is the check
which would have caught this round's gap on its own - a reference column with no counterpart and no reason
is a failure, not a paragraph.

### C. Teleport (`/tp`, `/teleport`) - measured, not blocked

The redirect is known: `TRANSFER_GAME_SERVER_INFO` (`0x01A6`, 46 bytes as GWPS sends it) then
`MAP_UPDATE_CURRENT` (`0x0099`, 12 bytes), both in `captures/zone-ref-005829.jsonl`. What is missing is
*authority*: only the client can start a travel today. Evidence: `docs/09` #4, `GameSrv.c:1360-1435`,
`docs/08-gm-commands.md`.

| # | item | done when |
|---|---|---|
| C1 | keep `account_id` (and the session) on the connection - state parity, `docs/09` #9 | the world can write a transfer row at all |
| C2 | a world-side `reserve` + `begin_transfer` for the target map | a transfer row appears with the right token and map |
| C3 | send `0x01A6` + `0x0099`; mark the connection *moved* so its teardown does not release the new lease | `/tp lakeside -> /tp ascalon` lands in Ascalon and the seat accounting is right |
| C4 | alias resolution and the reference's three refusals (unknown, no config, already here) | `/tp nonsense` prints the reference's line and sends nothing |

**C1-C4 landed.** `Play` carries the session and account, `/tp <alias>` (and `/teleport`) reserves the
destination's seat, mints a token, writes the transfer row and sends the pair; the connection is marked
*moved* so its teardown leaves the destination's seat alone. The test
`a_teleport_redirects_to_the_target_map_and_keeps_its_seat` drives the whole cycle: two refusals that send
nothing, the 46-byte redirect and 12-byte map update read by hand (the table's 5-byte form is the *login*
shape - the reference's "size per context" trap), the seat moving from one map to the other, a reconnect
with the handed-out token landing at the destination's own spawn point, and the old connection's exit
leaving the seat taken.

The one thing that is still this server's: it never constructs another world, so `/tp` reaches only the
maps it already hosts. The reference's auth service can place a player on any pre-spawned map server.

### D. Movement: validation, not just echo (T10)

ASURA echoes the client's destination, so the client's own prediction carries the walk and a wall is
not a wall. GWPS pathfinds (`GmPaths.c`, A* over the map's path graph from `Gw.dat`) and refuses what
it cannot reach. Evidence: `GmAgent.c:371-398`, `GmAgent.c:409-462`.

**D is sized, and two of its three units are now ported.** The reference's refusal is its pathfinder, and
the pathfinder is three units of C, all readable, plus the archive they read from:

| part | lines | what it is |
|---|---|---|
| `FaArchive.c` + `.h` | 204 + 72 | the `Gw.dat` archive: seek, read, MFT lookup by hash, and `FaDecompress` for compressed entries |
| `GmMapDataImport.c` + `.h` | 590 + 43 | the graph's chunk format: planes, trapezoids, vectors, root/X/Y/sink nodes, portal traps |
| `GmPaths.c` | 1,057 | trapezoid A*: `PathFindTrapezoid`, portal walking, waypoint reduction |

And the two things a port needs to be verified against both exist:

- **the data**: `Gw.dat` is present beside the reference at 3,655,804,520 bytes;
- **an oracle**: the reference ships built tools (`build/RelWithDebInfo/gwdat_pathing.exe`,
  `gwdat_scan.exe`) that dump the graph, so a ported reader can be compared message for message with the
  reference's own output rather than with a guess.

So `MOVE_TO_COORD` could not validate anything while those units were absent, and this was always a
**milestone, not an unknown**: the archive could be read and tested against `gwdat_pathing.exe`'s dump of
one entry before any A* was written - which is what D1 did, followed by D2 and D3 below.

What *can* be matched without it is matched, and asserted: the reference's position budget
(`speed_base * elapsed + tolerance`, clamped at 2 s of elapsed time) is the same rule on the same message, and
`a_move_is_echoed_as_sent_and_a_far_report_is_clamped` says so on the wire.

**D1 landed: the archive index.** `crates/asura-world/src/gwdat.rs` is the index half of `FaArchive.c` -
header, file table, TOC hash, entry lookup, and the stored bytes of one entry. No decompressor yet, so it
answers "is this file id in the archive and where", which is what the map catalogue depends on.

`gwdat-census` (an example) prints what the real archive says about itself:

```
header:  magic 0x1A4E4133, header_size 32, block_size 512, MFT at 3631642624 (3730656 bytes)
entries: 155444
  0x0001B97D  stored 1300044  compressed 8  decompressed 2925267  Lakeside County / Ascalon City / Ashford Abbey
  0x000345CC  stored  515444  compressed 8  decompressed 1734079  Kamadan, Jewel of Istan
```

Two independent checks come out of that. **2,925,267 bytes** for `0x1B97D` is the "2.9 MB region blob" the
reference's own roadmap describes; and every blob the generated catalogue names is present, which
`the_archive_index_finds_the_blobs_the_map_catalogue_names` asserts along with a bogus id being `None`.

A detail that had to be measured rather than read: `FILE_ARCHIVE_MAGIC` is the C multi-character literal
`'\x1ANA3'`, so the four bytes `1A 4E 41 33` collapse to `0x1A4E4133`, not to their byte-reversed reading -
the first attempt compared against the reversed value and failed against the real file.

**D2 landed: the decompressor.** `crates/asura-world/src/fa_compress.rs` is `FaCompress.c`'s decoder, ported
function for function - the MSB-first bit window (`ConsumeBits`/`GetBits`), `SetupNodesandTree`, `GetNextCode`
and `FaDecompress`'s block loop with literals and back-references. Its six static tables are generated from the
reference by `tools/gen-fa-compress-tables.py` (and `--check`ed in the battery), because the first hand
transcription of `Table1` produced values that appear nowhere in the reference.

It is verified against the archive, not against my reading of the C. `GwDat::file()` decodes an entry against
the size in its last four bytes - the pair `FaGetFile` performs - and `a_map_blob_decompresses_to_the_containers_own_signature`
checks three real map blobs for the things a one-bit error would break:

- the decoded length is exactly the announced size (Kamadan: 1,734,079 bytes from 515,444 stored);
- the first four bytes are `"ffna"`, which is the reference's `RIFF_SIGNATURE` (`GmMapDataImport.c:3`);
- the fifth byte is a chunk type, which its `RiffCheckHeader` also requires.

Two details the port had to get right rather than nearly right. The C reads its input as a `u32 *`, so a tail
shorter than a word makes it read the four bytes it was told to ignore - padding reproduces that without the
out-of-bounds read. And `next_bit_encoding + 1` relies on **unsigned wraparound** (the counter runs below zero
and wraps to `u32::MAX`); C wraps silently, Rust panics, so the port wraps explicitly with the reason in a
comment.

**D3 landed: the map importer.** `crates/asura-world/src/map_data.rs` is `GmMapDataImport.c` (590 lines)
a function at a time - `RiffCheckHeader`, `RiffParseChunks`, `RiffGetChunk`, `PathImport_CheckHeader`,
`PathImport_Step2`, and the ten plane readers (counts, tag 11, vectors, trapezoids, root node, x nodes, y
nodes, sink nodes, portal traps, portals) - plus `PathFindTrapezoid` from the top of `GmPaths.c`, which is
the query "which trapezoid is this position in".

The verification is the reference's own tool, not my reading of its C. `tools/gwdat_pathing.c` imports a
blob with `GmImportMap` and prints `file_id,size,planes,trapezoids,min_x,min_y,max_x,max_y`; if both sides
read the same file the same way, all eight numbers agree. They do, for every blob the catalogue names:

```
0x0001B97D,2925267,58,6120,-18432.000,-24576.000,21504.000,24576.000    Lakeside / Ascalon / Ashford
0x000345CC,1734079,39,1270,-18432.000,1726.000,9081.000,21504.000       Kamadan
0x0005CEE6,3320363,51,5246,-21504.000,-27648.000,21504.000,20736.000    The Catacombs
0x0005CEE7,3389269,18,5241,-24576.000,-21302.662,24576.000,25240.000    Wizard's Folly / Foibles Fair
0x0005CF29,3252319,45,4772,-23328.000,-21504.000,27648.000,16479.221    Green Hills / Barradin
0x0005CF2A,3713598,24,5267,-30720.000,-16608.000,26592.000,22168.000    Regent Valley / Fort Ranik
0x8005EA96,3657363,48,6890,-20640.000,-21504.000,24279.000,24331.000    The Northlands
```

Those are 283 planes and 34,806 trapezoids, and the odd-looking corners (`16479.221`, `-21302.662`) agree
to three decimals too - which is the point: a misplaced field or an off-by-one in a row size moves a
bounding box even when the counts happen to survive.

`map-census` (an example) is the runnable form: it imports all twelve catalogue rows out of a real
`Gw.dat` and prints each one's planes, trapezoids, node counts and where its spawn point lands, plus
`--fingerprint` for the eight-column shape above.

Three things make that a measurement rather than a claim:

- The reference's rows were held in `crates/asura-world/tests/fixtures/gwdat-pathing.csv`, which is gone from the tree with the rest of the parity fixtures (`docs/16`);
  it compared all eight columns of every catalogue blob against a real `Gw.dat`. The surviving
  equivalent is `the_spawn_points_the_reference_configures_land_in_the_graphs_in_the_archive`
  (`crates/asura-world/src/map_data.rs:1045`);
- `tools/parity-pathing.py --check` re-runs the reference binary, compares it to the fixture, and compares
  both to this repo's importer - the third leg is what catches the fixture going stale *and* the importer
  drifting in one run;
- `the_spawn_points_the_reference_configures_land_in_the_graphs_in_the_archive` walked every map's configured
  spawn point (`tools/map-config.sql`, generated from `GmMapsConfig.c`) and requires a trapezoid: all
  twelve land inside, three of them from the same blob (Lakeside County serving Ascalon City and Ashford
  Abbey as well).

Three reference behaviours were kept rather than corrected, and the reason is the same for all three:
they are what the file holds.

- `PathImport_PlaneTag11` advances `h000C * 8` bytes of a section whose declared length is `sub_length`,
  and in **all 283 measured planes** `sub_length` is exactly twice that. The next section's tag sits at the
  midpoint. A reader advancing `sub_length` would step over the vector section; the reference reads
  *inside* section 11's bounds and so does this port.
- `PathImport_Step2` takes its length from byte 1 of a five-byte header - an unaligned `u32` that
  swallows the tag byte. It is what the format is.
- `GetNodeFromDatId` answers a null pointer, not an error, for a node index past its array, so a graph can
  import with a dead end in it; `trapezoid_at` stops at a dead end like the reference's walk does.

Pointers became array positions (`NodeRef`, `Option<usize>`, `Option<(plane, portal)>`) - the same graph
without a self-referential struct.

**D3's other half landed: the search.** `crates/asura-world/src/pathfind.rs` is `GmPaths.c` and
`GmPathHeap.c` ported - the A* itself, the corridor/walk builder that turns a chain of trapezoids into
waypoints, the four neighbour visits and the two portal visits, and the heap. Its fixed-point
`PathReciproc`/`PathSqrt` tables (256 entries each, float *bit patterns* combined with integer
arithmetic) are generated from `GmPaths.c` by `tools/gen-path-tables.py`, because an A* fed by
`1.0 / x` and `x.sqrt()` is the same algorithm with different tie-breaks and therefore a different
walk.

**How it is measured.** `tools/path-oracle.c` is a host tool that compiles the reference's single
translation unit *unmodified* (`#define COMPILE_TOOLS 1`, the same trick the reference's own tools use)
and calls `PathFinding` directly; `tools/build-oracle.ps1` compiles it with the MSVC the reference
was built with, linking the static libraries its CMake build already produced. Nothing under
`PreSearing/` is written to. Both sides then run the same 96 cases
(a fixture file, `path-cases.txt` under `crates/asura-world/tests/fixtures/`, since removed with the other parity fixtures) and
print one CSV row per case with coordinates as raw IEEE-754 bit patterns:

```
the path finder matches the reference on 96 case(s): 41 walk(s) found,
127 waypoint(s) identical, 1 case(s) compared on the answer alone
```

So 95 of 96 walks - found or refused - come back with the same `result`, the same waypoint count, and
waypoints identical down to the last bit of every coordinate, including the paths that cross planes
through portals. The case-by-case comparison is
was `every_case_produces_the_references_own_waypoints` (`tests/pathfind.rs`); both the fixture and that
file are gone, and the surviving comparison is `tests/path_dump.rs` against `data/path-graphs.bin`, and
`tools/parity-path.py --check` re-runs the reference itself.

**The one case that cannot be pinned is a finding, not a gap.** `PathBuildAddWaypointAndReduce` reads `t`
and `s` after `FindIntersectionPoint` *fails* - the two lines are parallel, the function leaves its
outputs untouched, and the reference then branches on whatever was in those two stack slots. Exactly one
of the 96 cases has a step parallel to its own start-to-destination line, and the reference answers it with
11 waypoints on some runs and 12 on others (measured: two different answers for the same input, in the same
binary, differing only in one waypoint's y). No port can match an undefined value; leaving `t` at zero
reproduces the 11-waypoint answer byte for byte, so that is what the port does, and `tools/parity-path.py`
compares that case on its `result` alone and says so in the fixture's header.

Two other reference behaviours are kept for the same reason: `IsPointInTrap` runs its side tests on the
*absolute position* rather than the offset from the edge - so the region it accepts is a wedge from the map
origin, not the trapezoid's interior - and `PathAddNode` overwrites a trapezoid's node unconditionally and
queues it again on every visit.

**What D still owes the server.** The finder is measured but not yet *adopted*: `MOVE_TO_COORD` still echoes
the client's destination, and the reference instead pathfinds, refuses what it cannot reach, and walks the
agent waypoint by waypoint (`GmAgent.c:371-398`, `GmAgent.c:409-462`). Wiring it in changes behaviour the
load tests and the movement tests already pin, so it is the next unit.

### D4. Adopting the finder (`MOVE_TO_COORD` and the tick)

**D4 landed.** `MOVE_TO_COORD` now asks the finder, and the world walks agents itself:

- `lib.rs` keeps a `PathGraphs` - one imported graph per map plus one finder per map, behind one lock -
  loaded at startup from `Gw.dat` (`--gwdat`, `ASURA_GW_DAT`, then `Gw.dat`, then `..\PreSearing\Gw.dat`).
  That is the reference's `srv->map_context`, with one difference: the reference runs one server per map,
  so it has one context per process, while this hosts several maps in one process.
- The handler (`GameSrv_HandleMoveToCoord`) pathfinds from **the server's own position**, takes waypoint 0
  as the destination and broadcasts `AGENT_MOVE_TO_POINT`. On a refusal the reference cancels the walk and
  broadcasts `AGENT_UPDATE_POSITION`; **this tree cancels and sends nothing**, because that broadcast
  disconnected a live client once and snapped it into a wall once, both measured
  (`crates/asura-world/src/agent.rs:722-743`). The owner's ruling on whether the silence is the port is
  open (`crates/asura-world/tests/load.rs:3169`, ignored for that reason). An empty path sends nothing,
  as the reference's `0 < agent->waypoints.len` does.
- The world tick advances the agent `speed_base * delta` along the path (`GameSrv_UpdateAgentMovement`),
  hands over to the next waypoint when a segment ends - broadcasting the new destination as the reference's
  `GameSrv_UpdateAgentDestination` does - and ends the walk on arrival. `agent.rs` keeps that state as
  `Motion`, the reference's `destination`/`direction`/`speed`/`waypoints`/`waypoint_idx`.

What is asserted: `a_walk_is_answered_with_the_first_waypoint_of_the_servers_own_path` asks `World::graphs`
what the path is, sends a `MOVE_TO_COORD` that needs more than one waypoint, and requires the wire answer to
be waypoint 0 (and *not* the requested destination), the other player to see the same waypoint, and the
agent to have moved within a few ticks; `a_walk_the_finder_refuses_is_answered_with_the_agents_position`
sends one off the map's geometry and requires a position and no move message.

One deliberate difference, and the only one in the movement code: the reference divides by the distance when
it points an agent at a destination, so a walk to exactly where the agent stands gives it a NaN direction and
then a NaN position - which would be saved to the database and reloaded on the next visit. This keeps the
direction it had and lets the walk end immediately instead.

Still not integrated, and recorded rather than hidden: the reference's tick also regenerates
`health_per_sec`/`energy_per_sec` (`GmAgent.c:524-529`), which this sends once per load rather than once per
tick - invisible with no combat and no second player spending energy, and the audit's #3 row says so.

### E. Item lifecycle (T12)

`CREATE_UNNAMED_ITEM` cannot be emitted; the ground item cannot be sent; nothing picks anything up
(GWPS has no pick-up handler either).

**E2 landed.** `CREATE_UNNAMED_ITEM` turned out not to be a new feature but a *shape*: the reference has two item creators and uses both - `GameSrv_SendItemById` sends the named one for the inventory stream, and`GameSrv_SendItemsInBag` (`GmItem.c:111-127`) plus `GameSrv_BroadcastPlayerEquippedItems`
(`GmItem.c:129-160`) send the unnamed one for bag slots and equipped items. This repo had used the named form everywhere, so the creation screen's profession change was sending the wrong message. Fixed at the builder and at both call sites; the multi-player introduction now also broadcasts a newcomer's equipped items, visual equipment and cape in the reference's own order (`GameSrv.c:1643-1650`).

**E1 is decided, not done.** The ground item stays a log line: GWPS's own drop handler never removes the item and gives the ground agent the item's id as its model, and no capture contains a working drop. A capture is the only thing that would change this, and the reference cannot produce one - its handler is the unfinished one. Recorded in `docs/history.md`.

### F. The load block, byte-compared (T8, T15)

Only the item block is compared against a capture today. The player block is not - which is how two
missing party messages survived an audit that called the block equivalent.

| # | item | done when |
|---|---|---|
| F1 | a generated fixture from the capture (headers + sizes only, no names or UUIDs) | a test that fails when a message is dropped from the block |
| F2 | reconcile the block with the fixture | T15's audit is clean for the load |

**F1-F2 landed.** `tools/parity-load.py` reads the reference's own `GWPS_TRACE=1` log, drops the periodic
traffic (pings and simulation ticks, which the log has and a block dump does not - the reference's own "N
(non-feature)" tier), and compares the ordered `(header, size)` stream. Result: **480 messages, 0 missing,
0 extra, 0 size disagreements**. That measurement was committed as `crates/asura-world/src/load_fixture.rs`, which is gone from the tree
(headers and sizes only, no payload), `the_load_stream_is_the_references_message_for_message` compares the
four blocks against it, and `tools/test-battery.ps1` step 8 re-measures with `--check` so a drift fails the
battery.

The finding that started this item is closed by it: the two party messages are in the stream, in the
reference's position, at the reference's size.

**F3 landed: the payloads are compared too, and three messages differ for measured reasons.**
`tools/parity-load-bytes.py` walks the same 480 messages and compares the bytes - a wrong *value* is the same
size, which is exactly what a `(header, size)` diff cannot see. The result:

```
91 of 480 message(s) are byte-identical to the reference; 389 allowed difference(s) in 3 message(s)
```

The three, each with the reference's own code as the evidence:

- **`0x001A` `PVP_ITEM_STREAM_ADD` (387 messages).** The reference sends `params[0]` once per parameter:
  `pack_helper`'s nested-struct case (`msgpack.c:411-436`) calls **`unpack_helper`** - the other direction -
  and never advances `consumed`, so every element is read from the same place. The capture shows
  `0x24080081` three times for item 1, where the reference's own table has
  `{0x24080081, 0x25300102, 0x24B80300}`. This server sends the table's real values (generated from that same
  array by `tools/gen-pvp-items.py`), so the check compares the header, the item id and the count.
- **`0x0094` `MAP_UPDATE_UNLOCKED_LIST` (1).** 260 bytes of which the reference fills the list lengths and
  the bitmap copies of a fresh character - the rest is whatever the previous message left in its per-server
  buffer. The capture's payload carries armour file ids and dye colours in those bytes. This server sends
  zeros, and the check requires every byte it *does* set to be the reference's.
- **`0x001F` `WORLD_UPDATE_LOAD_TIME` (1).** A wall-clock measurement; only the size is comparable.

Everything else - the agent record, the player record, the whole item stream and the tail of the block - is
the reference's bytes. Two *dump* bugs surfaced on the way and are fixed: the example passed the default
traits, so `PLAYER_UPDATE_PROFESSION` sent 0 where the reference's character row held 1 (Warrior) and
`_UNLOCKED_PROFESSIONS` sent 0 where it held 2 - a difference in the dump rather than in the server.

`tools/pack-oracle.c` came out of the work and stays: it puts a message struct through the reference's own
`pack_msg` with the reference's format table (and prints that table's field list with `pack-oracle fields
<header>`), so a message's bytes can be asked for rather than reasoned about. `tools/build-oracle.ps1` now
builds either oracle.

### G. Chat (T11) - equal to the reference, and that is the ceiling

Both servers send a player's line and print a generated answer; neither has ever rendered chat on
screen. `0x005D`/`0x005E`/`0x0061` appear in no capture.

| # | item | done when |
|---|---|---|
| G1 | none, unless a capture of a working chat message appears | recorded, not worked |

### H. Operations parity (T13, T15)

**H1 landed as a measurement, all three clauses of it.** The cadence and the accounting are asserted by
`an_idle_pair_keeps_its_rhythm_and_its_count`: two connections idle for six seconds, the tick and ping on the
reference's own intervals, an idle client told nothing else, and the world's player list a pair that empties
when they leave. `a_ten_minute_idle_pair_keeps_its_rhythm` is the same assertion at the reference's own
duration - against a real **pair**, marked `#[ignore]` because a ten-minute test is a test nobody runs - and
it also **samples the working set** every fifteen seconds through `K32GetProcessMemoryInfo` and fails if it
grows by more than 8 MiB after the warm-up. The sampler checks itself first (16 MiB allocated and touched, and
the sample has to see it), because a flat line from an instrument that never moves is not a measurement.
The procedure, and the `ASURA_SOAK_SECONDS` short form, are in `docs/HANDOFF.md`.
| H2 | the generated audits run in `tools/test-battery.ps1` (dispatch, emit, state) and their output is in `docs/history.md` | a regression in any axis fails the battery |

## What is deliberately *not* in the queue

Recorded so nobody adds it later: matching GWPS where GWPS is unfinished or does not fit this design.

- **GWPS's drop handler** - `TODO` for partial stacks, item never removed, model id set to the item id.
- **GWPS's server-side chat** - gated off because it crashed its client twice.
- **`GmParty.c` hero support** (`PARTY_HERO_ADD`) - no heroes exist here; the captures carry it 0 times.
- **Download manifest data** (`INSTANCE_MANIFEST_DATA`) - 0 times in the captures; nothing to download.
- **Campaigns, PvP, guild halls** - the reference's own `X - Out of scope` tier.

## Order of work

1. **A1 + A2 + A3 + A4 + A5** - multi-player is what most of the missing surface is *for*; it also
   makes T9, T12 and T14 measurable for the first time.
2. **C1 -> C4** - `/tp` is measured end to end and unblocks the second map without a rebuild.
3. **F1 -> F2** - the load block becomes provable, which is what keeps the rest honest.
4. **B1 -> B4** - character creation, the largest data port.
5. **D1 -> D2 -> D3** (archive, decompressor, importer, finder - landed) -> **D4, adopting the finder in
   `MOVE_TO_COORD`** -> **E1/E2** -> **H1/H2**.

Each step lands with its test, and the audit documents are regenerated rather than edited:
`python tools/parity-report.py --write docs/history.md`, `python tools/parity-emit.py`.

## 11 - The state of parity, measured

> From `docs/history.md`, moved here on 2026-11-06.


> **History.** The tooling named below was retired when parity closed (docs/16); nothing here is runnable.

Produced by `tools/parity-summary.py`, which is gone from the tree. Every number here is the
**2026-10-31 reading** and cannot be re-derived: the generators that measured the axes and the fixtures
some of them compared against were retired with the parity tooling (`docs/16`). Where a number here
disagrees with a test or a file that still exists, that test or file is right.

## The axes

| axis | what it measures | state |
|---|---|---|
| dispatch | the reference's client-message handlers | auth **15/15**, game **18/18** - **complete**; 1 of them are no-ops in the reference itself |
| emit | every message the reference can send | **76 of 81** reachable, **0 real gap(s)**, 5 explained |
| the load block | `(header, size)` against the reference's own trace | **480 of 480** matched, **0 missing**, **0 extra**, **0 size** disagreement(s) |
| the load block's bytes | the same messages, payload against payload | **91 of 480 message(s) are byte-identical to the reference; 389 allowed difference(s) in 3 message(s), each with a recorded reason** |
| generated files | `--check` against the reference's sources | **12 of 12 current** |
| the map data | `Gw.dat` through the ported importer, against the reference's own tool | **the importer matches the reference on 7 blob(s): 283 plane(s), 34806 trapezoid(s), same bounding boxes** |
| the path finder | the same walks through both A*s, waypoint bits compared | **the path finder matches the reference on 96 case(s): 41 walk(s) found, 127 waypoint(s) identical, 1 case(s) compared on the answer alone** |
| the schema | every column `DbSchema.h` declares, against this database | **the schema accounts for all 95 reference column(s): 55 explained, 11 mapped, 7 packed, 22 stored** |
| tests | `cargo test --workspace` | **210 passed**, 1 failed at the 2026-10-31 reading; later partial runs are recorded in `docs/HANDOFF.md` |

## What the numbers say

The two lookup axes are complete: every client message the reference dispatches has an arm here, and
every message it can put on the wire is reachable from a send path here. The load block and the
generated tables match their sources message for message, and the seeds are regenerated from the same
sources rather than typed. The map-data axis is the odd one out: it measures a *port* against the
reference's own importer output for the same bytes, so a disagreement there is a reading bug, not a
missing feature.

## What is not measured here, and is written down elsewhere

Everything in this table is mechanical. The parts of parity that are not are enumerated in
`docs/history.md`, and the ones that remain are all of one kind - they need something this
machine cannot supply:

- **T10's search** is ported: `GmPaths.c`'s A* over the imported graph lives in
  `crates/asura-world/src/pathfind.rs`, click-to-walk answers with the first waypoint of this server's
  own path, and a walk the finder refuses is answered with the agent's position (`MILESTONES.md`,
  `docs/history.md`). What is not compared here is the finder's output against the reference's on
  live client walks: the recorded comparison covers 96 cases (`docs/history.md`).
- **T9 and T14's live halves** need a second real client rendering the other player.
- **T13's memory clause** needs a human watching the process during the ten-minute idle run.
- **The ground item** waits on a capture of a working drop, which the reference cannot produce because
  its own drop handler is the unfinished one.

## The generated checks

The twelve checks this table listed were `--check` runs of generators that are no longer in the tree
(`docs/09`, `docs/10`, `docs/16`). None of them can be re-run, so "current" is not a claim this page can
still make. What replaces them is narrower and real: the packet table is the implementation
(`crates/asura-protocol/src/packets.rs:34-40`), the migrations are versioned and applied by
`asura_db::migrations::ensure_current`, and the path regression is
`crates/asura-world/tests/path_dump.rs` against the bundled `data/path-graphs.bin`.

## 12 - The database schema, diffed against the reference's

> From `docs/history.md`, moved here on 2026-11-06.


> **History.** The tooling named below was retired when parity closed (docs/16); nothing here is runnable.

Produced by `tools/parity-schema.py` from `PreSearing/code/OpenTyria/DbSchema.h` and this database;
both that script and the reference checkout are outside the tree now, so the table is the **2026-10-31
reading**, not a live check. Every column the reference declares is either stored here, derived from
another table, or explained; a column that is none of those was the defect this page found, which is how
the character settings block was missed for as long as it was. Two rows have changed since: `last_outpost`
(`crates/asura-db/migrations/0024_last_outpost.sql`) and `eula_accepted`
(`0012_account_eula.sql`) are columns of their own now.

## What the reference declares, and where it lives here

### `session` -> `session`: 4 of 5 column(s) stored

| reference column | state | here |
|---|---|---|
| `user_id` | mapped | `session.portal_user` |
| `session_id` | mapped | `session.id` |
| `created_at` | mapped | `session.started_at` |
| `updated_at` | explained | no analogue: this schema closes a session with `ended_at` and never updates a row in place |
| `account_id` | mapped | `session.account_id` |

Columns this schema has that the reference does not:

- `portal_user` -- the portal identity the rig presents (`migration 0003`); the reference's `user_id`
- `char_id` -- the character this login selected; the reference keeps it on the account
- `world_id` -- the world a travel placed this session on
- `map_id` -- the map that travel asked for
- `ended_at` -- closing a session is what makes a second login win

### `account` -> `account`: 2 of 19 column(s) stored

| reference column | state | here |
|---|---|---|
| `account_id` | mapped | `account.id` |
| `created_at` | mapped | `account.created_at` |
| `updated_at` | explained | no analogue: account rows are written once |
| `eula_accepted` | stored | `account.eula_accepted` (`0012_account_eula.sql`), written when the client accepts (`crates/asura-auth/src/lib.rs:587-601`); the reference sends a hardcoded 24 (`AuthSrv.c:684`) |
| `current_char_id` | explained | kept on `session.char_id` here: the reference puts the last selected character on the account, this scopes it to the login that selected it |
| `current_territory` | explained | unobservable: read into the account row and never sent; a travel here is a `transfer` row |
| `storage_gold` | explained | not stored: `GameSrv.c:825` sends it in the storage-gold message, which this server sends from the character's own gold -- the reference never fills the storage bags either |
| `balthazar_points_max` | explained | fresh-account parity: this server sends the cap the reference sent for an untouched account (10000) in the four `FACTION_MAX_*` messages; the reference reads the column and nothing in it ever writes one |
| `balthazar_points_amount` | explained | zeros are parity: `GameSrv_SendPlayerFactions` reads it, and no handler in the reference assigns one |
| `balthazar_points_total` | explained | zeros are parity, as `balthazar_points_amount` |
| `kurzick_points_max` | explained | fresh-account parity: this server sends the cap the reference sent for an untouched account (10000) in the four `FACTION_MAX_*` messages; the reference reads the column and nothing in it ever writes one |
| `kurzick_points_amount` | explained | zeros are parity: `GameSrv_SendPlayerFactions` reads it, and no handler in the reference assigns one |
| `kurzick_points_total` | explained | zeros are parity, as `kurzick_points_amount` |
| `luxon_points_max` | explained | fresh-account parity: this server sends the cap the reference sent for an untouched account (10000) in the four `FACTION_MAX_*` messages; the reference reads the column and nothing in it ever writes one |
| `luxon_points_amount` | explained | zeros are parity: `GameSrv_SendPlayerFactions` reads it, and no handler in the reference assigns one |
| `luxon_points_total` | explained | zeros are parity, as `luxon_points_amount` |
| `imperial_points_max` | explained | fresh-account parity: this server sends the cap the reference sent for an untouched account (10000) in the four `FACTION_MAX_*` messages; the reference reads the column and nothing in it ever writes one |
| `imperial_points_amount` | explained | zeros are parity: `GameSrv_SendPlayerFactions` reads it, and no handler in the reference assigns one |
| `imperial_points_total` | explained | zeros are parity, as `imperial_points_amount` |

Columns this schema has that the reference does not:

- `username` -- the account name the rig logs in with
- `token` -- the login token the world validates a reconnect against
- `token_at` -- when that token was minted
- `last_ip` -- the address the last login came from

### `character` -> `characters`: 20 of 50 column(s) stored

| reference column | state | here |
|---|---|---|
| `char_id` | mapped | `characters.id` |
| `created_at` | explained | no analogue: rows are created once and `updated_at` carries the only timestamp this schema reads |
| `updated_at` | stored | `characters.updated_at` |
| `account_id` | stored | `characters.account_id` |
| `charname` | mapped | `characters.name` |
| `last_outpost` | stored | `characters.last_outpost` (`0024_last_outpost.sql`), written only for an outpost placement (`asura_db::game::remember_outpost`) |
| `last_guild_hall_id` | explained | zero: nothing here creates a guild, and the reference's own value for a lone character is the null uuid the settings blob carries |
| `sex` | packed | the packed `Appearance` word, `characters.appearance` |
| `height` | packed | the packed `Appearance` word, `characters.appearance` |
| `skin_color` | packed | the packed `Appearance` word, `characters.appearance` |
| `hair_color` | packed | the packed `Appearance` word, `characters.appearance` |
| `face_style` | packed | the packed `Appearance` word, `characters.appearance` |
| `hair_style` | packed | the packed `Appearance` word, `characters.appearance` |
| `race` | packed | the packed `Appearance` word, `characters.appearance` |
| `campaign` | stored | `characters.campaign` |
| `level` | stored | `characters.level` |
| `is_pvp` | stored | `characters.is_pvp` |
| `helm_status` | stored | `characters.helm_status` |
| `skill_points` | explained | zeros are parity: read by `GameSrv_SendPlayerFactions`, written by nothing -- no handler awards experience or points |
| `skill_points_total` | explained | zeros are parity, as `skill_points` |
| `experience` | explained | zeros are parity: read by `GameSrv_SendPlayerFactions`, assigned by no handler in the reference |
| `gold` | stored | `characters.gold` |
| `active_weapon_set` | explained | unobservable: `GameSrv_SendWeaponSlots` sends four zero slots and never reads the column |
| `primary_profession` | stored | `characters.primary_profession` |
| `secondary_profession` | stored | `characters.secondary_profession` |
| `unlocked_skills` | explained | zeros are parity: `GameSrv_SendUpdateUnlockedSkills` sends the bitmap, and the reference's own confirm writes it all-zero |
| `unlocked_maps` | explained | zeros are parity: stored and never read into a message |
| `completed_missions_nm` | explained | zeros are parity: stored and never read into a message |
| `completed_bonuses_nm` | explained | zeros are parity: stored and never read into a message |
| `completed_missions_hm` | explained | zeros are parity: stored and never read into a message |
| `completed_bonuses_hm` | explained | zeros are parity: stored and never read into a message |
| `unlocked_professions` | stored | `characters.unlocked_professions` |
| `skill1` | explained | zeros are parity: `GameSrv_SendSkillbarUpdate` sends them, and every writer in the reference leaves them zero |
| `skill2` | explained | zeros are parity: `GameSrv_SendSkillbarUpdate` sends them, and every writer in the reference leaves them zero |
| `skill3` | explained | zeros are parity: `GameSrv_SendSkillbarUpdate` sends them, and every writer in the reference leaves them zero |
| `skill4` | explained | zeros are parity: `GameSrv_SendSkillbarUpdate` sends them, and every writer in the reference leaves them zero |
| `skill5` | explained | zeros are parity: `GameSrv_SendSkillbarUpdate` sends them, and every writer in the reference leaves them zero |
| `skill6` | explained | zeros are parity: `GameSrv_SendSkillbarUpdate` sends them, and every writer in the reference leaves them zero |
| `skill7` | explained | zeros are parity: `GameSrv_SendSkillbarUpdate` sends them, and every writer in the reference leaves them zero |
| `skill8` | explained | zeros are parity: `GameSrv_SendSkillbarUpdate` sends them, and every writer in the reference leaves them zero |
| `file_id_body` | explained | derived: the settings blob reads the piece's file id from the item in that equipped slot |
| `file_id_legs` | explained | derived: the settings blob reads the piece's file id from the item in that equipped slot |
| `file_id_head` | explained | derived: the settings blob reads the piece's file id from the item in that equipped slot |
| `file_id_boots` | explained | derived: the settings blob reads the piece's file id from the item in that equipped slot |
| `file_id_gloves` | explained | derived: the settings blob reads the piece's file id from the item in that equipped slot |
| `colors_body` | explained | derived: the settings blob reads the dye colours from the item in that equipped slot |
| `colors_legs` | explained | derived: the settings blob reads the dye colours from the item in that equipped slot |
| `colors_head` | explained | derived: the settings blob reads the dye colours from the item in that equipped slot |
| `colors_boots` | explained | derived: the settings blob reads the dye colours from the item in that equipped slot |
| `colors_gloves` | explained | derived: the settings blob reads the dye colours from the item in that equipped slot |

Columns this schema has that the reference does not:

- `pos_x` -- where the character stood, saved on leaving
- `pos_y` -- as `pos_x`
- `pos_plane` -- as `pos_x`

### `bag` -> `bag`: 4 of 7 column(s) stored

| reference column | state | here |
|---|---|---|
| `account_id` | explained | derived: every bag query here is scoped by character, which implies the account |
| `char_id` | mapped | `bag.character_id` |
| `created_at` | explained | no analogue: rows are written once, at creation |
| `updated_at` | explained | no analogue: bags are not updated |
| `bag_model_id` | stored | `bag.bag_model_id` |
| `bag_type` | stored | `bag.bag_type` |
| `slot_count` | stored | `bag.slot_count` |

Columns this schema has that the reference does not:

- `bag_id` -- the number the client is told; per character, as the wire wants it
- `assoc_item_id` -- the item that *is* the bag (`bag_item_id` in the reference's in-memory model)

### `item` -> `item`: 10 of 14 column(s) stored

| reference column | state | here |
|---|---|---|
| `account_id` | explained | derived: every item query here is scoped by character, which implies the account |
| `char_id` | mapped | `item.character_id` |
| `bag_model_id` | explained | stored as `item.bag_id`, a reference to the bag row rather than a copy of its model id |
| `slot` | stored | `item.slot` |
| `created_at` | explained | no analogue: rows are written once, at creation |
| `updated_at` | explained | no analogue: an item is rewritten, never dated |
| `quantity` | stored | `item.quantity` |
| `dye_tint` | stored | `item.dye_tint` |
| `dye_colors` | stored | `item.dye_colors` |
| `item_type` | stored | `item.item_type` |
| `profession` | stored | `item.profession` |
| `model_id` | stored | `item.model_id` |
| `file_id` | stored | `item.file_id` |
| `flags` | stored | `item.flags` |

Columns this schema has that the reference does not:

- `materials` -- `CREATE_NAMED_ITEM`'s materials field
- `unk1` -- `CREATE_NAMED_ITEM`'s unexplained field, kept so the message can be rebuilt
- `value` -- `CREATE_NAMED_ITEM`'s value
- `name` -- the item's name, as the UTF-16 units the wire carries
- `modifiers` -- the item's modifier list, as the wire carries it

## What this does not claim

The database is the least observable surface this project has, and a column being stored here is not
the same as a feature working. What the check does claim is narrower and worth more: no column of the
reference's schema has been forgotten, and every one that is absent is absent for a recorded reason.

The two mappings a reader should look at first are `session.user_id` -> `session.portal_user` (the
portal identity the client presents, which the reference calls `user_id`) and `character.charname` ->
`characters.name` (UTF-16 units on the wire, text here).

## 13 - The GM command surface, diffed against the reference's

> From `docs/history.md`, moved here on 2026-11-06.


> **History.** The tooling named below was retired when parity closed (docs/16); nothing here is runnable.

Produced by `tools/parity-commands.py` from `GmCommands.c`, `GmChat.c` and `GmMaps.h` against this
repo's command table and its alias table; the script is gone from the tree, so this is the 2026-10-31
reading rather than a live check. The command surface is small and its blast radius is not: `/tp`
moves a player between servers, `/maps` is the only way to find out what a world hosts, and `/stuck`
sends a different message from the cancel that looks like it. All three are implemented
(`crates/asura-world/src/chat.rs`, `crates/asura-world/src/travel.rs:181`).

## The commands

| what a player types | where the reference handles it | takes an argument | what this server answers |
|---|---|---|---|
| `/tp`, `/teleport` | `GmCommands.c:184 GameSrv_TryHandleGmCommand -> GameSrv_HandleTeleportCommand` | yes | a transfer, or the reference's refusals |
| `/maps` | `GmCommands.c:206 GameSrv_TryHandleGmCommand -> GmCommand_ListTargets` | no | the alias table in the log |
| `/stuck` | `GmChat.c:243 GameSrv_HandleChatCommand` | no | the agent's position |

The reference's names, from its two dispatch points: `/tp` (GmCommands.c), `/teleport` (GmCommands.c), `/maps` (GmCommands.c), `/stuck` (GmChat.c).

## The `/tp` aliases

`s_teleport_targets` is twenty aliases over twelve maps: two for most, because the reference lets a
tester type `lakeside` or `lakeside county`. This server resolves through the same generated table,
so every one of them works -- which it did not until the table was generated: matching on each
map's *name* accepted six and answered "unknown map name" for the rest.

| alias | label | map | type | hosted here |
|---|---|---|---|---|
| `ascalon` | Ascalon City (pre-Searing) | 148 | 0 | yes |
| `ascalon city` | Ascalon City (pre-Searing) | 148 | 0 | yes |
| `lakeside` | Lakeside County | 146 | 1 | yes |
| `lakeside county` | Lakeside County | 146 | 1 | yes |
| `ashford` | Ashford Abbey | 164 | 0 | yes |
| `ashford abbey` | Ashford Abbey | 164 | 0 | yes |
| `catacombs` | The Catacombs | 145 | 1 | yes |
| `the catacombs` | The Catacombs | 145 | 1 | yes |
| `northlands` | The Northlands | 147 | 1 | yes |
| `the northlands` | The Northlands | 147 | 1 | yes |
| `green hills` | Green Hills County | 160 | 1 | yes |
| `barradin` | The Barradin Estate | 163 | 0 | yes |
| `barradin estate` | The Barradin Estate | 163 | 0 | yes |
| `wizards folly` | Wizard's Folly | 161 | 1 | yes |
| `foibles` | Foibles Fair | 165 | 0 | yes |
| `foibles fair` | Foibles Fair | 165 | 0 | yes |
| `regent` | Regent Valley (pre-Searing) | 162 | 1 | yes |
| `regent valley` | Regent Valley (pre-Searing) | 162 | 1 | yes |
| `fort ranik` | Fort Ranik | 166 | 0 | yes |
| `kamadan` | Kamadan, Jewel of Istan | 449 | 0 | yes |

`hosted` is this server's answer to "can we actually serve that map": the reference refuses a
target with no `GmMapConfig` rather than letting its loader fall back to Kamadan, and this refuses a
map the world was not started with. Both refusals are logged, and neither puts an answer on the
wire -- the reference's chat gate is off in both servers, so a GM answer has always been a log
line.

## 14 - The reference's test plan, mapped onto the checks in this repository

> From `docs/history.md`, moved here on 2026-11-06.


> **History.** The tooling named below was retired when parity closed (docs/16); nothing here is runnable.

Generated by `tools/parity-testplan.py` from `PreSearing/docs/04-scope.md` and this repository's own
test functions and tools. The reference's plan is the closest thing either project has to a
definition of the observable surface: sixteen items, each with a stated pass criterion. This page
says, for each one, which check here covers it -- and every named check is verified to exist, so a
renamed test breaks the check rather than quietly rotting the page.

**How these checks are used is governed by `docs/HANDOFF.md` #"Testing policy":** they are a
regression net for shared machinery and milestones. A scoped change is tested by its own targeted
check and by the live client, not by running every suite (owner's standing instruction, 2026-10-28).

```powershell
python tools/parity-testplan.py --check
```

| state | items |
|---|---|
| measured | 13 |
| hand | 2 |
| blocked | 1 |

`measured` is a check in this repository. `hand` needs a rig run or a human watching two windows, and is
recorded with whatever automated half exists. `blocked` needs a capture the reference cannot produce,
because its own handler is the unfinished one. `absent` is a component the reference has and this
stack does not -- named, explained, and not pretended away.

## T0: Build and seed (measured)

**The reference's pass criterion:** `gwps_server.exe` produced; `db/database.db` exists.

**Here:** the battery builds and migrates; the schema's own version table is asserted

- `tools/test-battery.ps1` -- a tool
- `crates/asura-db/src/migrations.rs` -- a tool

## T1: Web gate works (no client needed) (measured)

**The reference's pass criterion:** an XML `<Reply><Session>GUID</Session></Reply>` comes back with a valid UUID. This validates the login front without involving the game client at all.

**Here:** the gate is this project's own `crates/asura-webgate`, a port of the reference's `webgate.py` answered from the database rows `tools/seed-dev.ps1` writes; the four calls, their exact XML and the 400/401 refusals are asserted, and the `(UserId, Token)` pair it returns is the pair the auth service resolves (the live-database half is pinned by `asura_db::portal`)

- `the_four_endpoints_answer_what_the_reference_answers` -- a test
- `the_http_framing_round_trips` -- a test
- `an_unknown_email_is_400_who_are_you` -- a test

## T2: Server starts and binds (measured)

**The reference's pass criterion:** three `AuthSrv listen to '127.0.0.1:6112/6113/1337'` lines, process stays alive, Ctrl+C exits.

**Here:** both services bind and answer; the battery's steps 6 and 7 run them

- `a_client_logs_in_and_receives_the_whole_login_batch` -- a test
- `a_client_logs_in_travels_and_reaches_its_map` -- a test

## T3: Handshake (measured)

**The reference's pass criterion:** server logs the connection and completes key exchange without `Unhandled` or `Invalid riff type` errors; client proceeds to the login screen past the version check.

**Here:** key exchange, framing and the version message are asserted at the protocol layer

- `a_client_logs_in_and_receives_the_whole_login_batch` -- a test
- `an_unknown_header_is_an_error_rather_than_a_guess` -- a test

## T4: Account login (measured)

**The reference's pass criterion:** account row loaded from DB (no `Failed to open database`, no `Couldn't load the account`); client reaches EULA or character list.

**Here:** the login decision, its refusals included

- `a_presented_pair_resolves_to_its_account` -- a test
- `a_closed_session_cannot_log_in` -- a test

## T5: EULA + character list (measured)

**The reference's pass criterion:** EULA accepted (`ACCEPT_EULA` path), character list renders (empty on a fresh DB is fine).

**Here:** the EULA is accepted at the protocol level and the character list is the batch's first message

- `a_client_logs_in_and_receives_the_whole_login_batch` -- a test
- `a_character_list_is_scoped_to_its_own_account` -- a test

## T6: Character creation (measured)

**The reference's pass criterion:** all of `CHAR_CREATION_REQUEST_PLAYER`, `_REQUEST_ARMORS`, `_CHANGE_PROF`, `_CONFIRM` are handled with no `Unhandled GameSrv packet`; a row appears in `characters`, plus `bags` and `items` rows for that character. Verify directly: ```powershell python -c "import sqlite3;c=sqlite3.connect('db/database.db');print(c.execute('select count(*) from characters').fetchone(), c.execute('select count(*) from bags').fetchone(), c.execute('select count(*) from items').fetchone())" ```

**Here:** the creation instance, the four handlers, and the rows the confirm writes

- `a_character_is_created_in_a_creation_instance` -- a test
- `the_character_settings_come_back_from_the_row_the_creation_wrote` -- a test

## T7: Character select -> map transfer -> spawn (measured)

**The reference's pass criterion:** auth server creates a game server (`AuthSrvCtrl_CreateServer`), the game server loads Kamadan (`GameSrv_LoadMap`), the client receives the spawn message and renders the world with the player standing at the configured coordinate `(-9067, 13218)`.

**Here:** travel, the redirect, the load block and the spawn point, end to end

- `a_client_logs_in_travels_and_reaches_its_map` -- a test
- `a_placed_client_is_loaded_into_its_map` -- a test
- `a_teleport_redirects_to_the_target_map_and_keeps_its_seat` -- a test

## T8: No crash on the only configured map (measured)

**The reference's pass criterion:** log contains no `No config for map ... falling back to default` for Kamadan, and `Invalid riff type` does not appear. This confirms `map_file_id 0x345CC` and the archive reader agree.

**Here:** the archive reader, the decompressor and the importer, against the reference's own tool

- `the_archive_index_finds_the_blobs_the_map_catalogue_names` -- a test (`crates/asura-world/src/gwdat.rs:358`)
- `a_map_blob_decompresses_to_the_containers_own_signature` -- a test (`crates/asura-world/src/gwdat.rs:310`)
- `every_catalogue_blob_imports_to_the_references_own_fingerprint` -- retired with its fixture
  (`tests/fixtures/gwdat-pathing.csv`); the surviving comparison is
  `the_spawn_points_the_reference_configures_land_in_the_graphs_in_the_archive`
  (`crates/asura-world/src/map_data.rs:1045`)

## T9: Second client sees the first (hand)

**The reference's pass criterion:** each client sees the other player's agent; both see movement of the other.

**Here:** the server half is asserted (introduction, withdrawal, movement broadcasts); *seeing* the other agent needs a second real client

- `a_second_player_is_introduced_to_the_first_and_withdrawn_again` -- a test

## T10: Movement and pathfinding (measured)

**The reference's pass criterion:** movement is accepted, the other client sees it smoothly, and invalid destinations are rejected rather than teleporting the player.

**Here:** the walk is the reference's own path, refused the reference's way, and the finder matches it case by case; the client's *smoothness* is the hand half

- `a_walk_is_answered_with_the_first_waypoint_of_the_servers_own_path` -- a test
- `a_walk_the_finder_refuses_is_answered_with_the_agents_position` -- a test, now `#[ignore]`d: the handler
  sends nothing on a refused walk and the owner's ruling on which side is right is open
  (`crates/asura-world/tests/load.rs:3169`)
- `every_case_produces_the_references_own_waypoints` -- retired with the 96-case fixture; the surviving
  comparison is `tests/path_dump.rs` against `data/path-graphs.bin`

## T11: Chat (hand)

**The reference's pass criterion:** both clients display it; no `Unhandled` for `SEND_CHAT_MESSAGE`.

**Here:** a spoken line is sent as the reference sends it; whether a client *renders* it is the hand half, and neither server has ever managed it

- `the_chat_messages_a_line_produces_are_the_references_pair` -- a test

## T12: Item drop (blocked)

**The reference's pass criterion:** `DROP_ITEM` handled, item entity appears/disappears consistently for both clients, DB state still coherent.

**Here:** the reference's own drop handler is unfinished (a TODO for stacks, no removal, the item's id as the model id) and no capture holds a working drop; this server deducts and sends nothing, which is measured, and the ground item waits on a capture that does not exist

- `a_drop_deducts_the_stack_and_says_nothing` -- a test
- `a_drop_sends_nothing_because_both_candidates_crashed_a_client` -- a test

## T13: Stability (measured)

**The reference's pass criterion:** no crash, `PING_REQUEST`/`PING_REPLY` cycle continues (`GAME_SRV_TIME_BETWEEN_PING_MS = 5000`), memory roughly flat.

**Here:** cadence, accounting and the working set over ten minutes, all asserted: the long variant samples the process every fifteen seconds and fails on more than 8 MiB of growth, after self-checking the sampler with a touched 16 MiB allocation

- `an_idle_pair_keeps_its_rhythm_and_its_count` -- a test
- `a_ten_minute_idle_pair_keeps_its_rhythm` -- a test

## T14: Disconnect and reconnect (measured)

**The reference's pass criterion:** the game server notices the disconnect, removes the agent, and a fresh login succeeds. **Known risk:** the auth server keeps `GlobalAccountRegistry` in RAM (`AuthSrv.h:156`) and there is no graceful shutdown, so a *server* restart (rather than a client reconnect) is expected to misbehave - record it, don't fix it in Phase 0.

**Here:** disconnect, the agent removal, and a fresh login

- `a_character_comes_back_where_it_left` -- a test
- `a_closed_session_never_resolves_again` -- a test

## T15: Log audit (measured)

**The reference's pass criterion:** every occurrence is understood and recorded as a known-missing feature (not a bug). Anything unexpected becomes a finding.

**Here:** the log audit is generated rather than grepped: dispatch, emit surface and the state page

- `tools/parity-report.py` -- a tool
- `tools/parity-emit.py` -- a tool
- `tools/parity-summary.py` -- a tool

## Starting ASURA's services: research for a decision

> From `docs/history.md`, moved here on 2026-11-06.


> **Status: research, and the decision it asked for is made.** The evidence below produced
> `crates/asura-supervisor`; the retraction at the end of this block records where that decision went.
> This document exists so that the decision - *how should ASURA's services be started, supervised and
> found?* - was made on evidence rather than from first principles.
>
> **Where the decision went (2026-10-28):** the level-2 supervisor it recommends was built
> (`crates/asura-supervisor`) and its decisions are recorded in `docs/history.md` (S1
> and D1, S2 and D2), not as ADR 0011 - the owner judged an ADR unnecessary once every decision was
> written down there. The launcher scripts this document describes as "today's" state
> (`tools/start-server.ps1`, `play.ps1`, `stop-server.ps1`) were retired on 2026-10-28; the one run
> path is `docs/19-running.md`, on Windows and Linux.
>
> Sources: the owner's direction ("we need some way of starting microservices, so we don't have a
> single monolithic service providing us services"), the GDC 2017 talk *Guild Wars Microservices and
> 24/7 Uptime* (transcription at `PreSearing/build/gw2-text.txt`, distilled rules at
> `docs/history.md`), and the state of this repository at `ea00f60`.

## The question

ASURA already runs as separate operating-system processes, so the thing to fix is not "split one
service into many". The question is narrower and more useful:

**What should "start the stack" actually be - and what should it guarantee?**

When this was written, "start" was a set of PowerShell scripts, and everything after "the processes are
running" was
assumed rather than provided: nothing restarted a crashed service, nothing answered "is the auth
service healthy?", nothing drained a world except the fields a world itself writes, and the only
coordination a service could announce itself through was the `world` table (which speaks for world
servers only). The supervisor below was built for exactly those four gaps, and closed them.

## Plain words, so the decision can be discussed

These words recur in the talk and in the design; each one is defined here so that a conversation
never depends on knowing them already.

| word | what it means here |
|---|---|
| **service** | one program with one job (`asura-gateway`, `asura-world`, `asura-webgate`), or the database. "Service" does not imply "small" - a world server is a large service. |
| **process** | one running copy of a program. Two world servers on a machine are two processes of one service type. |
| **supervisor** | a program whose only job is to start other programs, watch them, restart them when they die, and stop them cleanly. It is not part of the game; if it dies, the game keeps running until the next restart. |
| **orchestrator** | a program that decides *which machine* each process runs on, starts new builds, moves traffic, and shuts old builds down. A supervisor manages processes on one machine; an orchestrator manages machines. |
| **registry / service catalog** | a table (ours would live in the database) listing what services exist, where they are, what build they are, and whether they are alive. Consumers look things up instead of hard-coding addresses. |
| **manifest** | one file in the repository that names every service, its executable, arguments, ports and dependencies - the description a launcher or supervisor reads. Written by hand, versioned with the code. |
| **health check** | a cheap question with a yes/no answer ("are you ready?"), asked on a timer or at startup, used to decide whether to restart, advertise or drain. |
| **drain** | stop accepting new work but finish what is in hand, then exit. For a world: no new players, existing players keep playing until they leave. |
| **rolling deploy / concurrent builds** | a new build starts alongside the old one, new traffic goes to it, and the old one is drained and stopped. Players notice nothing; nothing is switched off just to switch on. |

## What the source says

The GDC 2017 talk (Stephen Clarke-Willson, ArenaNet) - slide numbers from
`docs/history.md`, which cites them so claims can be checked.

- **The definition** (slides 12-13): microservices are *asynchronous and separately restartable*;
  most of their services have process-level isolation, because restartability is much harder without
  it. This is the only part that matters when deciding what a service boundary is for: the boundary
  exists so that one thing can die and be rebuilt without the others.
- **What their estate looks like** (slide 15): about 80 types of services, but not 80 programs -
  some are libraries hosted by `ArenaSrv.exe` or `JsSrv.exe`, and `SpawnSrv.exe` hosts processes.
  Hosts listen on one port and delegate connections. So "many services" is a property of the design,
  not a requirement to ship many executables.
- **Orchestration** (slides 25-27): their definition is "allocation of services to hardware,
  permissions, routing, isolation". Configuration is a file per service: a permission section, a
  mapping from groups to IPs, changed by humans, not automatically. The minimum useful configuration
  is `127.0.0.1: all` - one line that lets the whole stack run on a laptop.
- **Registration** (slide 30): routing is *configuration plus self-registration*. Many services
  self-register with `RouteSrv`. "In an ideal world there would only be self-registration (and
  advertising over DNS)" - which the speaker explicitly says was not their reality. This is the
  honest version of service discovery for a company with a private datacenter: a registry table plus
  a config file, and humans make the changes.
- **SpawnSrv** (slides 19-21): starts the new build, routes new traffic to it, ensures it does not
  fall over, then shuts down the old build. Several builds run concurrently.
- **What "run your own" actually costs** (the datacenter slide): restarting all services at once
  (cold start), IP addresses and DHCP, automatic deployment, OS images, health monitoring, backup
  and restore. The talk frames these as complexity to challenge you - a research checklist, not
  features to copy blindly.
- **Restartability** (Part Four): authoritative services must be accurate about reality; RAM state
  is a fast cache and usually not authoritative; the pattern that works is *rebuild by asking*, not
  *reload from disk* (the GW1 tournament server keeps no state at all); where state must survive,
  two copies that do not restart together (GW2 WvW: load balancer plus maps). The retry rules and
  the newest-connection rule are already adopted in `docs/history.md`.
- **The game map server is a *macroservice*** (slide 18): nearly all the features of a microservice
  (dynamic configuration and deployment, multiple simultaneous builds) except independent
  restartability. This is the permission to keep "one world server = one big process".

## What existed when this was written, and what replaced it

The table is the state the research found: three PowerShell launchers, no restart, no shared log, no
registry row for the gate or auth. **None of those scripts is in the tree**, and the replacement is the
supervisor below them.

| piece | how it starts | health | restart if it crashes |
|---|---|---|---|
| PostgreSQL | a Windows service or a systemd unit, managed elsewhere; the supervisor checks it and waits, never starts it | a reachable connection | OS service recovery, outside this repository |
| `asura-webgate` :6601 | `asura-supervisor start`, from `config/services.toml` | a TCP connect on the port | bounded retry: 5 attempts, 2 s apart, then FAILED |
| `asura-gateway` :6112 | the same | a TCP connect on the port | the same |
| `asura-world` :6113 (world `8001`) | the same; registers itself in `world`, heartbeats every 5 s, publishes `world_pool` capacity | the world's own row is fresh | the same |

Facts worth carrying into the decision:

- **The `world` table is already a miniature registry** - id, address, port, build, draining,
  population, heartbeat - and placement already refuses stale or draining worlds. The talk's
  registration model exists, but for one kind of service.
- **The `build` and `draining` columns are SpawnSrv's idea, and they have their pilot now**:
  `find_worlds` reads both, `set_draining` writes one, and `asura-supervisor deploy-world --old --new`
  performs the rolling deploy (`crates/asura-db/src/placement.rs:439-463`,
  `crates/asura-supervisor/src/supervise.rs:525`).
- **The socket layer is also the game-connection front door** (`crates/asura-gateway/src/relay.rs`):
  the real client dials the auth address regardless of what `GAME_SERVER_INFO` names, and auth
  routes the socket to the world it handed out. That is a routing responsibility sitting inside a
  service the architecture describes as "not in the data path".
- **The start is a script, and the scripts are the operational weak point**: port checks fail
  with "an earlier run is probably still up", logs go to `%TEMP%`, stopping is by process name, and
  a crashed world is simply gone until someone notices a client cannot connect.
- **A crashed service is restarted, and status is aggregated**: the supervisor retries at most
  `restart_max` times and then reports FAILED, and `asura-supervisor status` answers what is running
  and how fresh each service is (`crates/asura-supervisor/src/supervise.rs:355-411`,
  `status.rs`). Counters the architecture still asks for are not exported - that paragraph of
  `docs/02-architecture.md` remains unbuilt.

## The decision space

Four levels, ascending. Each one includes everything the previous level does. The names are
deliberately plain: they describe what the programs *do*, not products.

### Level 1 - one launcher, one manifest

A single command reads a manifest (which services, arguments, ports, dependencies, log files) and
starts them in the right order. It replaces `start-server.ps1` / `play.ps1` and makes "what runs"
visible in version control. **Buys:** one entry point, correct order, no duplicated port logic.
**Costs:** little. **Does not provide:** restart, health, drain.

### Level 2 - a supervisor

A small process (a Rust binary in this workspace is the natural choice, `asura-supervisor`) that
spawns every service from the manifest, restarts a crashed one with limited retries, starts/stops
one service without touching the others, drains and stops cleanly, and keeps one log per service.
**Buys:** the thing the current scripts are missing most - a dead world or gate comes back by
itself, and a world can be stopped without killing the login path. **Costs:** one more program to
maintain, and a rule: the supervisor must never be load-bearing for correctness (if it dies, the
services run on; it restarts itself like anything else). **Leaves open:** discovery (who finds
whom) - addresses still come from the manifest.

### Level 3 - supervisor plus a service catalog

Every service type registers in the database (a `service` table in the spirit of `world`), or keeps
its existing registry row (worlds) while the gate and auth get theirs. Consumers read the catalog;
the front door can route from it instead of hard-coded ports. **Buys:** the talk's registration
model generalized; services discover each other without config, and a service that moves (a world
on another port or machine) is a row, not a config edit. **Costs:** a new table and its lifecycle
rules (stale rows, reaping - the same machinery `world` already proves). **Leaves open:** nothing
structural; still one machine, or many - addresses are in the table.

### Level 4 - an orchestrator

Placement of service instances onto machines, concurrent builds with traffic switching, permission
isolation, health-based replacement - the SpawnSrv/RouteSrv picture at the scale of a real
datacenter. **Buys:** rolling upgrades of every service without downtime, and horizontal scale-out.
**Costs:** the whole checklist from the talk's datacenter slide (cold start, addressing, images,
monitoring, backup), plus a control plane that must itself stay up. **This is a decision about the
deployment, not about the code** - the protocol already survives a world living elsewhere
(`docs/history.md`, step 5).

### Sub-decisions the levels do not settle

1. **Granularity.** Keep webgate + auth + world(s) + database, or split further? Candidates that the
   design already hints at: pull the front-door routing out of auth (a tiny proxy service, or the
   client pointed at the world and the auth telling it *who it is* only); a content/file service
   (`Gw.dat` streaming, `docs/02-architecture.md`); later chat/presence if they ever exist.
   `docs/history.md` already argued for *deliberately coarser* services - this decision
   re-opens that on purpose or leaves it alone, and either way it should say so.
2. **Who starts the database.** Today the script starts the Windows service (elevation). A
   supervisor could own it, but the DB is the one process whose death interrupts everything
   (`docs/02-architecture.md`, failure table), so its supervision is an operational choice, not just
   another manifest line.
3. **Restart policy per service.** Auth: restart freely (players reconnect). World: restart and
   re-register (players on it lose their session, as today). Database: restart only deliberately.
   The talk's rule applies: rebuild by asking, never by reloading state from disk.
4. **What "healthy" means** per service, and which check a supervisor uses: the port is open (a
   process exists), the schema is current (world and auth both refuse stale schemas at startup -
   `asura_db::migrations::ensure_current`, `crates/asura-db/src/migrations.rs`), a heartbeat is recent (worlds), or an HTTP probe
   (webgate). A health check that only tests "the port is bound" is a false comfort; one that asks
   "can you answer one real request?" is what placement already does when it dials a world before
   offering it (`docs/roadmap.md`, M4 progress).

## Where ASURA already matches the talk, and where it does not

| their design | ours today | gap |
|---|---|---|
| login separate from worlds | `asura-gateway` and `asura-auth` vs `asura-world` | none |
| worlds self-register; consumers read the registry | `world` + `world_pool`, heartbeat, reaper, reconciler | the model is right; only worlds have it |
| restartable worlds, disposable and interchangeable | a world rebuilds from the database; a dead one is skipped and reaped | nothing restarts a dead one automatically |
| build + drain described in the registry | `build` and `draining` columns exist | nothing performs a build or a drain |
| newest connection wins | adopted as rule 1 | none |
| retry only timeouts, 4-6 attempts, ~30 s | adopted as the retry policy | none |
| metrics cheap; rates not resets | specified in `02-architecture.md` | no exporter; no reader |
| SpawnSrv: start new build, switch traffic, verify, stop old | - | absent; this is the launcher/supervisor/level-3 work |
| RouteSrv: self-registration is the routing | partial (`world` only) | the gate and auth do not register anywhere |
| many small services, one job each | three kinds of service, deliberately coarse | a stated choice, not an accident - revisit consciously |

## Open questions the research must answer before an ADR

1. **What deployment is the target?** One machine (a private server on a home box) or several
   machines? This single answer decides whether levels 3-4 are ever needed, and it is the first
   thing to establish - everything else is cheap on one box, and orchestration is not.
2. **What must "robust" mean, concretely?** Rank the guarantees: (a) a crashed world comes back;
   (b) a crashed auth comes back; (c) an upgrade does not kick players; (d) I can see that everything
   is alive from one place; (e) services can be started individually. Each has a different owner
   (supervisor, auth design, drain protocol, status endpoint) and a different cost.
3. **Where does the front-door responsibility live?** Inside auth (today), as its own tiny service,
   or moved to the world (client pointed at the world, token validated by it)? Any supervisor /
   orchestrator design inherits this answer, because the front door is what a client dials first.
4. **Does the service catalog grow from `world`, or is `world` the template for a general table?**
   The lessons doc warns against turning the database into a network service layer; a catalog is
   less than that, but the warning should be answered, not ignored.
5. **What does the supervisor itself rely on?** The talk's rule - never a design where two
   components must restart together - means the supervisor must be startable, restartable and
   killable without any other service noticing. Its own state should be rebuilt by asking, like
   everything else.

## Suggested next research steps (not decisions)

1. Read how AzerothCore launches and supervises `authserver` and `worldserver` (its realmlist
   registration is the ancestor of our `world` table; their `.conf` files are the ancestor of a
   manifest) and write down the three things it does that our scripts do not.
2. Sketch the manifest for today's four services (one page): fields, order, ports, restart policy,
   log paths, and what a "cold start" of the whole stack means with a database that must be up
   first.
3. Translate SpawnSrv's loop to this schema: the world already advertises `build` and `draining`;
   the missing half is who starts `build` B, who switches traffic, and who stops build A. Write the
   smallest sequence that performs it.
4. Decide (with the owner) only the first open question - target deployment - and record it. The
   rest follow from it, and the ADR can then be written with a level in mind.

## What this document does not do

It does not choose a level, define the `service` table, write a supervisor, or re-open any recorded
ADR. `docs/history.md` still stands; this is the research its "Not adopting" section said
would be revisited only on purpose. If the owner's direction changes the shape of this brief, the
brief changes - the decision stays the owner's.

## Implementation plan: the seven improvements from the analysis

> From `docs/history.md`, moved here on 2026-11-06.


> **Status: delivered.** S1, S2, S3, S5a, S6 and S7 are delivered; S5b and S4 are closed. Every item
> the analysis listed has its stamp below. This plan contains **only** what the owner-approved
> analysis lists under "What we can improve" - seven suggestions, no eighth. Every item below quotes
> the suggestion it implements. If something is not in that list, it is not in this plan.

## The rules this plan is written under (from `docs/HANDOFF.md`, binding)

- **Do exactly the command.** No additions "while we're here".
- **The pre-flight per change:** one sentence - what changes, in which message or code path, and why
  it is not simply copying the reference; the source of truth named with `file:line` or bytes; if it
  contains anything not asked for, ask instead.
- **One change, one subject.**
- **The real client is the test; the suite is not a gate.** Test what the change touches, nothing
  else. No growing scaffolding around a small change.
- **Report, do not decide.**
- **Additive:** no wire changes; schema changes are new numbered migrations, never edits; every
  existing script keeps working until a phase proves its replacement, with the owner's word.

## The mapping: suggestion -> item -> evidence -> proof

| analysis suggestion ("What we can improve") | item | evidence it is real today | done-when |
|---|---|---|---|
| **1.** Replace the scripts with a launcher and a babysitter | S1 *(delivered 2026-10-28)* | `play.ps1` runs a `while` loop that throws when either child exits; args hard-coded; logs in `%TEMP%` | one command starts and stops the stack; a crashed local service comes back; real client login/travel unchanged |
| **2.** Make the database's service registry general | S2 *(delivered 2026-10-28)* | `world`/`world_pool` exist and are load-bearing; `asura-webgate` and `asura-auth` register nowhere | every service kind has a registry row; a consumer reads it; live-database proof |
| **3.** Give the `build`/`draining` columns a pilot | S3 *(delivered 2026-10-28)* | both columns exist (`crates/asura-db/src/placement.rs`); `register_world` writes `build` (`asura-world/src/main.rs:286`); `find_worlds` never consults it (`placement.rs:435`); nothing reads `draining`; a world cannot be stopped without a kill | a new build takes new players, the old world drains without kicking anyone, proven with the real client |
| **4.** Add a status that tells the truth | S4 *(closed 2026-10-28: not built - what the operating layer owns is already reported, and nothing else is in its scope)* | `docs/02-architecture.md` asks for counters and rates; nothing exports per-service counters, and the operating layer is not where they belong | a status view of the operating layer: what is running, is it healthy, heartbeat age, capacity, uptime |
| **5.** Untie the receptionist knot; fix the stale documents | S5a *(delivered 2026-10-28)* + S5b *(closed 2026-10-28)* | `crates/asura-auth/src/frontdoor.rs` is the router inside auth; the docs claims in S5a are false today | the five corrections land (S5a); a recorded decision on the front door (S5b) |
| **6.** Prove multi-machine capability on one machine, no hardware | S6 *(delivered 2026-10-28)* | the front door dials the **registered** address (`frontdoor.rs:82-96`), so cross-host routing already exists on paper; nothing has ever run at a second address | a travel to a world registered at a second loopback address, recorded |
| **7.** One "how do I run this" file, and CI's place | S7 *(delivered 2026-10-28)* | README's run instructions are stale (see S5a); `tools/test-battery.ps1` is the local battery; the repo's testing policy says the suite is not a gate | one accurate run document; a CI decision recorded (or none, with the reason) |

## S1 - Launcher and babysitter (the supervisor)

**The suggestion.** "Replace the scripts with a launcher and a babysitter."

**What existed before this item, as it read on 2026-10-28.** `tools/start-server.ps1` (elevation for
PostgreSQL, started the gate, handed over), `tools/play.ps1` (seeded, built, started auth and world with
`Start-Process`, tailed two `%TEMP%` logs, threw the moment either child exited),
`tools/stop-server.ps1` (killed by process name). All three were retired by this item; none is in the
tree. There was
no manifest, no state file, no per-service log directory, no way to start or stop one service
alone, and no restart.

**What the item delivers (nothing beyond the suggestion):**

- one command that starts the local stack in the right order, and one that stops it;
- supervision of the local services: a crashed one is restarted (the "babysitter");
- one log file per service, in one place;
- the list of services read from the config folder (`config/`), so what runs on this machine is
  data, not code;
- the old scripts keep working throughout; retiring them is a separate step with the owner's word.

**Delivered 2026-10-28.** `crates/asura-supervisor` (a Rust binary: `start`, `stop`, `status`,
`restart`, `build`, `schema`, `seed`), the list in `config/services.toml`, one log per service under
`logs/`, a loopback control channel, bounded restarts (5 attempts, 2 s apart, then FAILED - D1.3),
the port rule, and the process-group rule so Ctrl+C stops services *through* the supervisor.
Verified by 12 tests (10 unit + 2 integration) and a live run: start -> all three ready -> status ->
stop -> every port free. The cross-platform `schema`/`seed` commands live in the same crate.

**Settled by the owner** (recorded in their words, 2026-10-28):

- **It is a supervisor** (a service manager). It starts this machine's services and watches them -
  not an orchestrator. Across machines the database remains the only coordinator (S6), so no
  orchestrator is needed.
- **Configs live in one folder.** A dedicated config folder in the repository (`config/`) holds this
  machine's service list - the supervisor's "recipe" - and any future non-secret configuration, so
  nothing is scattered. Secrets (the database URL) stay in `.env`, gitignored, as today.
- **Windows and Linux.** The manager is a Rust binary so one codebase runs on both; the current
  scripts are Windows-only. Verifying it on Linux needs a Linux environment (WSL, a container, or
  CI); until then Linux is built but untested, and the run document says so.
- **When the manager exits, it stops what it started, and nothing else.** The database is out of
  its scope: checked and waited for, never started (a Windows service on Windows, systemd on
  Linux - "managed elsewhere").
- **A restart stops the old process first**: if it does not exit, kill it; wait until its port is
  free; then start the new one. No new process binds while the old one might still hold the port.

**Open decisions - D1 (the rest):**

- the restart policy when a local service crashes (retry a few times, then report it dead - the
  proposal - or retry forever).

**Must not change.** The services' command-line flags, their wire behaviour, the log lines the
owner reads.

**Proof (the testing policy):** the new command starts and stops the stack; killing one local
service makes it come back; the real client still logs in and travels. Targeted tests only for the
supervisor's own new behaviour.

**Risk.** Process control on Windows: the settle rule above (stop the old one, wait for its port,
then start the new one) is exactly what the current scripts lack.

## S2 - A general service registry

**The suggestion.** "Make the database's service registry general - every service announces itself,
not just worlds."

**What exists.** `world` (id, name, address, port, build, draining, population, heartbeat) and
`world_pool` (per-map capacity/leases) are proven and load-bearing: placement and the front door
read them. `asura-webgate` and `asura-auth` register nowhere; their addresses live in scripts.

**What the item delivers:**

- a registry every service kind can write itself into, in the same database, on the same pattern as
  `world`;
- a reader (placement, the front door, or status) that consumes it;
- worlds keep their existing rows and the proven `world_pool` machinery (they are not moved).

**Delivered 2026-10-28.** Migration 0010 adds `service` (kind, name, address, port, build, state,
started_at, heartbeat_at; unique per kind+name); `asura-db::catalog` holds the upsert, list, state
and delete queries; the supervisor publishes a row for every service it starts - kind from the
config or the name prefix, address from `--bind` - every 5 s, and a final publish on exit leaves
every row at `stopped`; `asura-supervisor status` prints the registry table with STALE marking.
Verified against the live database: the catalog test (register/refresh/staleness/stop/delete) and a
live run (rows for auth, webgate and world-8001, fresh; stopped after `stop`).

**Decisions - D2 (settled by the owner's go-ahead):** a separate catalog beside `world` (the proven
world machinery stays untouched), and the supervisor writes a row for each service it starts.

**Must not change.** `world`/`world_pool` semantics and the atomic reservation; the one rule
(never offer a client something that is not answering).

**Proof:** live-database test - a row is written, read back, and a stale row is reported as stale;
the consumer uses it. No client involvement.

## S3 - A rolling deploy (build id + drain)

**The suggestion.** "Give the upgrade columns an operator - a rolling deploy: start the new build
beside the old one; nobody gets kicked; a player keeps their build until the client reconnects (on
their next travel, or one we push - a transfer is a travel); and a draining world ends when its last
player has disconnected."

**What exists.** `world.build` is written as `asura-world/{version}`
(`crates/asura-world/src/main.rs:286`); `world.draining` is written by nothing and read by
placement (`placement.rs:432`) and by nothing else; `find_worlds` orders by `leased, id`
(`placement.rs:435`) and never looks at `build`; a world cannot be stopped without a kill, and
killing it leaves its row for the reaper.

**What the item delivers:**

- a way to run a new build beside the old one on one machine (no hardware);
- a placement rule that sends new players to the new build;
- a drain that lets the old world finish its players and leave - proven with the real client on the
  old world: nobody is kicked;
- the decision and the evidence recorded (the columns are no longer vocabulary without an operator).

**Decisions - D3 (settled by the owner, 2026-10-28):**

- where the new build runs: **a second world process beside the old one**; nobody gets kicked;
- a player's migration: **they keep playing until they need to reconnect by any means; on that
  reconnect they are moved to the new build** - no pushed transfers, no client restart (a transfer
  is a travel: the client tears down, reconnects, lands on the new build with a loading screen);
- placement among builds: **the newest build serves new arrivals and reconnects; the old build
  serves only the players already inside it, and a full new world refuses rather than falling back
  to the old build** (the one rule already refuses instead of hanging);
- how a draining world ends: **when its last player has disconnected** - no forced kick, no timeout
  that kills players.

**Delivered 2026-10-28.** `crates/asura-world/build.rs` stamps each build with
`git describe --always --dirty` (overridable with `--build`/`ASURA_BUILD`); migration 0011 adds
`world.started_at`, set on every registration, so "newest build" means "the build of the most
recently started fresh world hosting the map"; `find_worlds` offers only that build (a full newest
build refuses; no fallback to an older one - proven by `the_newest_build_is_the_only_one_offered`
against the live database); the world's heartbeat now publishes its real population and leaves the
registry and exits once a draining world's last player has disconnected; the supervisor gained
`deploy-world --old N --new M`, which starts the new world, proves it is registered, and only then
marks the old one draining. Verified live on one machine: two worlds on ports 6113/6114 both
registered with the same git build id; `deploy-world 8001 8002` reported ok; world 8001 drained and
its row disappeared without a kick, world 8002 stayed fresh. The real-client half (a player standing
in the old world while the deploy runs, and a reconnect landing on the new build) is the owner's
test with the client.

**Must not change.** The one rule: a client is never sent to something that is not answering. The
wire. The reaper stays the safety net for ungraceful deaths.

**Proof (the real client, per the testing policy):** a client in the old world is not interrupted by
the deploy; a new client's travel lands on the new build; a targeted database test proves the
placement rule.

## S4 - System health: keepalives and the status view (closed, not built)

> **Closed by the owner, 2026-10-28: not to be built.** The operating layer's liveness is already
> reported where it belongs - `asura-supervisor status` prints each local service's state, pid and
> restart count, and the registry table marks every row fresh or STALE with its age. What S4 would
> add beyond that (heartbeat age as a number on fresh rows, `world_pool` capacity/leases, the
> supervisor's uptime) is display polish, not a capability the operating layer lacks. Per the owner's
> scope ruling, the operating layer does system keepalives only - no player data, no metric table -
> and the services that own player numbers keep them in their own terms. The section below stays as
> the record of the scoping; nothing in it is being implemented.

**The suggestion.** "Add a 'status' that tells the truth about the operating layer - what is running,
is it healthy, and is it alive - and shows the *rate* of anything cumulative rather than a total
that resets. Player-domain data is not this item's business: the services that own it keep it in
their own terms."

**What exists.** `asura-supervisor status` prints the local process table (state, pid, restart
count) and the service registry with a STALE marker; the world server's registry row carries its
build, draining flag and population, and `world_pool` carries the per-map capacity and leases. What
it does not show: how old each heartbeat is, the capacity/leases of a world, and how long the
supervisor itself has been up. Nothing exports per-service counters, and nothing should from here.

**What the item delivers:**

- one command that answers, about the system: what is running, is it healthy, how long since its
  last heartbeat, what capacity it publishes and how much is leased, and how long the supervisor
  has been up;
- the world's own population and build are shown **because the world reported them** (its registry
  row) - a read of what the service said, never the operating layer measuring players;
- the *rate* form for anything cumulative that lives in this layer (restart counts; nothing else
  is a counter here - the supervisor starts and stops processes, it does not count gameplay);
- the one write it must not do: no metric table, no counters from the operating layer. The
  operational numbers that already exist live in the registry (world row, `world_pool`); the
  player-domain ones (logins, travels, moves, ticks) stay inside the services that own them.

**Scoped by the owner, 2026-10-28:** "everything regarding the player needs to go in the world
service, not here - I understand keepalives and metrics for the systems, but not player data." So
this item is the operating layer's health view only. The auth service and the world server keep
their own numbers in their own terms (their log lines today; whatever they do with them is their
item, not this one).

**Decisions - D4 (collapsed by the scope):** no new table, no counters in the operating layer. The
status view reads the registry and `world_pool`; the world's population is displayed as the world's
own report.

**Must not change.** The simulation loop's timing; the wire; the existing logs.

**Proof:** start the stack -> status shows each service with its state and heartbeat age; stop one ->
it shows the stop; a world that leaves the registry disappears from the view; the supervisor's
uptime is shown. A targeted check, live. No player-domain number is produced by the operating
layer.

## S5a - Fix the stale internal documentation

**The suggestion** (the second half of improvement 5). "...fix the stale documents to match reality."

**What exists.** Five claims in the repository's own documents are false today. Each is a
`file:line` fact, not a preference, so each correction is a factual change to internal
documentation:

| claim | where | truth |
|---|---|---|
| "the worlds stay in C and keep simulating" / "the world servers remain the existing C implementation" | `README.md` (~line 57, #Language) | `crates/asura-world` is the live world in Rust; M4 was delivered with the real client |
| "placement prefers the newest healthy build" | `docs/02-architecture.md` (deployment section) | `find_worlds` orders by `leased, id`; `build` is not consulted (`placement.rs:435`) |
| "each world server and the auth service exposes a small fixed set of counters ... and a rate form" | `docs/02-architecture.md` (observability section) | nothing exports anything |
| "a world server ... marks itself draining or offline on exit" (flags = online) | `docs/02-architecture.md` (the registry section) | no exit path, no flags column; the row is left for the reaper |
| "6 migrations applied / 11 tables" | `docs/HANDOFF.md` (environment status) | nine migration files (`0001`-`0009`) existed when this was written; the tree now carries 24 (`0001`-`0024`) |

**What the item delivers:** the five corrections, one subject per change, each verified against the
tree before the edit. This is internal documentation only.

**No open decisions.** Every correction is a fact; if the owner disputes one, that correction is not
made, and the claim stays until the owner decides.

**Must not change.** Code, behaviour, the wire - this item touches documents only.

**Proof:** each corrected line points at the `file:line` that proves the correction; `git diff`
shows documents only.

**Delivered 2026-10-28.** The five claims are corrected in `README.md`, `docs/02-architecture.md`
and `docs/HANDOFF.md`, each citing its evidence; the change touches documents only.

## S5b - The front-door decision (closed by the owner, 2026-10-28)

**The suggestion** (the first half of improvement 5). "Untie the receptionist knot - decide whether
routing belongs inside login, as its own tiny program, or inside the world."

**What exists.** `crates/asura-auth/src/frontdoor.rs` routes the game connection from the auth
service to the world, dialing the world's **registered** address. A measured fact decides the
options: the client dials the address it was launched with for its game connection, regardless of
what `GAME_SERVER_INFO` names - so "the world opens its own door and the client dials it" is not
available without changing the client.

**Closed by the owner (2026-10-28):** the relay is the **auth server's job**. "If you want to do
microservices you can, but they are part of the auth server" - no gateway process, no second
component: the only program at the address the client dials is the auth server, so it relays, as it
already does. A gateway would be the auth server under another name, with one more process and a
client-facing re-proof. What the item did leave behind is the correction of the claims that said
otherwise: `docs/02-architecture.md` (three places) and `docs/03-protocol.md` #11 now state the
measured fact and its consequence (the auth service is in the gameplay data path; a restart of it
drops the connections it relays).

**Must not change.** The wire; the client path.

**Proof:** the decision is recorded; the false claims are corrected; no code changed.

## S6 - Multi-machine capability, proven without hardware

**The suggestion.** "Prove the two-machine hop - on one machine, no hardware, since that's the
deployment."

**What exists.** The front door already dials the registered world address
(`frontdoor.rs:82-96`), and `asura-world --public` exists - so a world at another address is
"configuration" by design, and unproven in fact. Nothing has ever run at a second address.

**What the item delivers:**

- a second world, genuinely separate, registered at a **second loopback address** (all of
  `127.0.0.0/8` is loopback on Windows and Linux), so the registry, the front door and the client
  treat it as a world on another machine;
- a recorded experiment: a travel to it lands; the drain crosses it; the limits are written down
  (same OS, same NIC, no latency/NAT/firewall - a real LAN adds those, operationally, not in the
  design).

**Open decisions - D6:**

- the exact scenario (a second world at `127.0.0.2`; anything more is the owner's call).

**Must not change.** Nothing: this item changes no code unless the proof finds a defect (and then
the defect is the change, restored to the reference's behaviour where applicable).

**Proof:** the recorded run, with the client, plus the test that the front door used the registered
address and not the bind address.

## S7 - One "how do I run this" file, and CI's place

**The suggestion.** "A real 'how do I run this' in one file, and CI so the test battery runs without
a human remembering."

**What exists.** The README's run section is stale and scattered; `tools/test-battery.ps1` is the
local "everything" command. The repo's testing policy (`.HANDOFF.md`, 2026-10-28) says the real
client is the test and the suite is not a gate - which is directly relevant to what "CI" should be
here.

**What the item delivers:**

- one document that says, in order, what to run, what each command needs, and what success looks
  like (it replaces the stale run instructions and is verified against the tree);
- a recorded CI decision.

**Open decisions - D7:**

- CI: none (the local battery remains the tool), lint/format only, the suite on a runner with a
  database, or the suite nightly. The testing policy's "the suite is not a gate" argues against
  gating feature work; the owner decides what "CI" means before the item starts.

**Must not change.** The testing policy. The battery's usefulness.

**Proof:** the run document is followed end to end by someone who has not run the stack; CI, if
chosen, does exactly what the decision says and no more.

**Delivered 2026-10-28. D7.1 settled by the owner: format + lint only.** The one run document is
`docs/19-running.md` - what you need once, the one command (`asura-supervisor start`), what success
looks like, the everyday commands, the rolling deploy, the client, CI, and a troubleshooting table;
it is verified against the tree (ports from `config/services.toml`, the command list from
`crates/asura-supervisor/src/main.rs`). `README.md` and `docs/HANDOFF.md` now point at it instead of
repeating instructions. CI is `.github/workflows/ci.yml`: `cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets -- -D warnings` on every push and pull request - no
database, no test suite, per the testing policy. The gate needed the tree to be clean, so the five
clippy warnings it surfaced (two dangling doc comments left by the parity tooling's removal, and two
lints in `tests/path_dump.rs`) were cleared first; the Linux runner doubles as the first Linux
signal for the supervisor, which has not had a live Linux run yet.

## Ordering (dependency order, not preference)

1. **S5a - the stale-document corrections** - factual, each backed by `file:line`; no code, no
   decisions. *(delivered 2026-10-28)*
2. **S1** - the launcher/babysitter everything else builds on. *(delivered 2026-10-28)*
3. **S2** - the service registry; a consumer for S4. *(delivered 2026-10-28)*
4. **S3** - the rolling deploy (needs S1 to run two worlds; decisions settled). *(delivered 2026-10-28)*
5. **S4** - the operating layer's health view (needs S2's registry to read). *(closed 2026-10-28: not built - liveness is already reported by the supervisor and the registry)*
6. **S6** - the multi-machine proof (needs S1/S3 to run and drain a second world). *(delivered 2026-10-28: docs/history.md)*
7. **S5b - the front-door decision** - closed: the relay is the auth server's job, and the claims that said otherwise are corrected.
8. **S7** - the run document anytime; CI last, with the testing policy in view. *(delivered 2026-10-28: docs/19-running.md + .github/workflows/ci.yml)*

## Decisions needed (answer each before its item starts)

| id | item | decision | options | proposal (rejectable) |
|---|---|---|---|---|
| D1.1 | S1 | what the launcher is | Rust binary in this workspace / one improved script / third-party manager | **settled**: the supervisor - a Rust program, Windows and Linux |
| D1.2 | S1 | how it learns what to run | a config file in the repo / `.env` / command-line flags | **settled**: a dedicated config folder (`config/`) with this machine's service list inside |
| D1.3 | S1 | restart policy for a crashed local service | restart with a fixed retry cap, then report / restart always / no automatic restart | restart with a fixed cap; no exponential backoff |
| D2.1 | S2 | the registry shape | one general table / a separate catalog beside `world` (worlds unchanged) / worlds only, others stay in config | separate catalog beside `world` - additive; `world` stays load-bearing |
| D2.2 | S2 | who writes a row | the service itself / the launcher on its behalf | the launcher for the services it starts; worlds keep writing their own |
| D3.1 | S3 | where the new build runs | a second world process beside the old / restart in place | a second world process - additive, no kick |
| D3.2 | S3 | placement among builds | newest build first, older if newest is full / newest only / no build rule yet | newest first - the architecture already says so |
| D3.3 | S3 | how a draining world ends | the world notices `draining` and exits when empty / the launcher stops it after a wait / the registry row goes stale | the world exits itself when empty - same mechanism on one machine and across |
| D4.1 | S4 | where the numbers live | a table written by the services / an endpoint on each service / the launcher reads what exists | **closed**: not built - the launcher reads what exists (the registry), and nothing new is written |
| D5.1 | S5b | the front door | **closed**: the relay stays in the auth server; no gateway, no second component | - |
| D6.1 | S6 | the proof scenario | a second world at `127.0.0.2`, client travel to it / more | the second world at `127.0.0.2` |
| D7.1 | S7 | CI's place | none / lint+format only / the suite on a runner with a database / nightly | **settled**: format + lint only - `cargo fmt --check` and clippy; no database, no test suite |

## What this plan does not contain

- Any gameplay work (NPCs, combat, quests, items) - not in the analysis's improvement list.
- Any hardware requirement - every item is provable on the one machine everything already runs on.
- Any schema, format, or policy that was not asked for: the decisions table is the only place those
  appear, and each is the owner's.
- Any date or effort estimate: the owner's time is the cost, and the analysis did not price it.

## Multi-machine capability, proven on one machine (S6)

> From `docs/history.md`, moved here on 2026-11-06.


> The design says a world server is interchangeable and can live on another host - the client is
> told a real address, and the front door dials the **registered** address, so moving a world should
> be configuration, not code. This document proves that, without hardware, and states what a real
> second machine would still add.

## The claim, and its parts

| part | where it is |
|---|---|
| the world registers where a *client* reaches it (`--public`), separate from where it binds (`--bind`) | `crates/asura-db/src/placement.rs`, `register_world` |
| placement offers worlds by their registered address | `find_worlds` |
| the auth front door dials the **registered** address of the world the client named | `crates/asura-auth/src/frontdoor.rs` |
| a world at another address can be drained and leaves by itself | `SET draining` -> the world's heartbeat loop (S3) |

## The experiment on one machine

All of `127.0.0.0/8` is loopback on Windows and Linux, so `127.0.0.2` is a genuinely different
address to every component: the registry stores it, the front door dials it, and the world binds
it. It is the single-machine equivalent of a second host.

Config used (two worlds on the box, the second at the "other machine" address):

```
[services.world-8001]
args = ["--world", "8001", "--capacity", "8", "--bind", "127.0.0.1:6113", "--public", "127.0.0.1:6113"]

[services.world-8002]
args = ["--world", "8002", "--capacity", "8", "--bind", "127.0.0.2:6114", "--public", "127.0.0.2:6114"]
```

With `ASURA_TRACE=1` on the auth service:

1. **Both worlds register.** `SELECT id, address, port FROM world` ->
   `8001 | 127.0.0.1 | 6113` and `8002 | 127.0.0.2 | 6114`, both fresh.
2. **A game connection naming world 8002 arrives at the auth address** (127.0.0.1:6112 - the client
   always dials the launch address; see `docs/02-architecture.md`). The auth trace says:

   ```
   auth: routing the game connection to world 8002 (asura-dev) at 127.0.0.2:6114
   ```

   and the world at `127.0.0.2` receives it:

   ```
   world <- game version build=39112 world=8002 map=146 player token 0x12345678
   ```

   The front door could only dial `127.0.0.2` because that is the **registered** address - it never
   sees a world's bind address. (The probe had no pending transfer, so the world then refused it,
   which is the correct answer for a connection nobody issued a token for.)
3. **The second-address world drains across the "machine" boundary.** Marking it draining:
   `world: drained; the last player has left; leaving the registry`, its row disappears, world 8001
   is untouched. Placement, routing and the drain all treat the address as data.

## What a real second machine would add, and what it would not

- **Would add:** network latency, NAT and firewall operation, DNS or a realmlist-style address, and
  the operational work of starting the second supervisor with its own `config/services.toml`. None of
  that is protocol or design work.
- **Would not add:** a second auth service, a gateway, a new message, or a client change. The client
  dials its launch address; the front door dials the registered world address. A world on host B is
  one `--public` value and one registry row.
- **The one rule holds:** the client is only offered a world that is answering (the auth dials the
  candidate port before it is offered).

## Evidence to fill in with the real client

The last step is the owner's: point the real client at the auth address with a second-address world
registered, stand in a map on `127.0.0.1` (world 8001), travel to a map served by `127.0.0.2`
(world 8002), and confirm it lands - then the same with a real second host when one exists.

| step | expected | observed |
|---|---|---|
| both worlds register at their own addresses | `8001@127.0.0.1`, `8002@127.0.0.2` | yes (2026-10-28, one machine) |
| front door dials the registered address | trace names `127.0.0.2:6114` | yes (2026-10-28) |
| second-address world drains and leaves | row removed, the other world untouched | yes (2026-10-28) |
| real client travels between the two | lands, no kick | _to run_ |

## The packet-parity attempt: objective, what was done, and why it failed

> From `docs/history.md`, moved here on 2026-11-06.


Written 2026-10-31, after reverting the working tree to `e96a909 "up to date and working"`.

Every tool, fixture and file this page names lives on that branch, not on `main` unless the text says
otherwise; the few things that were later taken from it are marked. The work itself is not lost: it is
committed on the branch **`packet-parity-attempt`** (`9818bf2`). `main`
is the working version, and this document lives on `main` so the failure is not the only thing that
survives.

## The objective

The owner's words, which are the specification: *"reproduce faithfully the packet stream of the live
server/client"*. The instruction given at the start of this attempt: *"need you to fix the packets then,
notify when client is ready to get tested, instrument the logging obsessively so we can troubleshoot after if
needed"*.

Nothing in that asks for new features, new world content, or a redesign of how messages are built. It asks
for the byte stream to match the reference, and for the server to be able to say what it sent.

## What was done

**Measured the reference, at the wrong layer.** Every server-to-client size the captures under
`recordings/client/` contain was collected into a generated table (`crates/asura-protocol/src/live_sizes.rs`,
`tools/gen-live-sizes.py`), and the world compares every frame it sends against it at run time. Those sizes
are the client's message struct, not the wire (`docs/03-protocol.md` 5e), so the table said "the reference's
size" about numbers that are 2 bytes or more above what the reference sends. The run-time line it prints is
worth keeping; its reference is not.

**Corrected sizes and field widths, message by message.** Around thirty messages were brought to the
sniffer's shape, each with its captured bytes recorded next to the row in
`crates/asura-protocol/src/packets.rs`: `0x0028`, `0x0099`, `0x00F1`, `0x017C`, `0x0196`, `0x019A`,
`0x00DA`, `0x00DB`, `0x001D`, `0x0018`, `0x001A`, `0x01A0`, `0x0029`, `0x01A6`, `0x006E`, `0x008B`,
`0x009F`, `0x00A2`, `0x00B6`, `0x00F0`, `0x0186`, `0x0189`, `0x018E`, `0x01D3`, `0x01D4`, `0x01CC`,
`0x01B3`, `0x0022`, `0x00E9`, `0x0020`, and others. Two conclusions came out of it, one measured and one
wrong: that the live server writes **one element fewer than it counts** on `0x001A` and `0x01A0` (126 and
10 samples, from the same capture layer, so it needs a wire confirmation before it is a rule), and that a
game message's header is four bytes - which was the client's struct read as the wire, and is what "What was
actually wrong" below is about.

**Measured the order.** `tools/stream-diff.py` compares our stream with a live one. It showed the
exploration grid and the instance table arriving in the wrong place, and both were moved to where the
captures put them.

**Built the instrumentation the instruction asked for.** The world records every message of every session to
`logs/capture-world-8001.jsonl`; each session ends with a `stream summary` line; `tools/stream-sizes.ps1`,
`tools/stream-diff.py`, `tools/client-check.ps1`, `tools/live-gaps.py` and `tools/gen-npc-spawns.py` read
those files back. `docs/25-client-readiness.md` and `docs/26-packet-gaps.md` describe the state and the
remaining work.

**Built one thing that was not asked for.** A harvested NPC population (`npc_spawns`), switched off by
default behind `ASURA_NPC_POPULATION`. It is on the branch and it is not part of the objective.

## The failure

**The client still answers `007`.** That was true before the attempt, and it was still true after the last
build of it (2026-10-31 16:25). The last run did reach a served session and was not usable from it: the
client loaded into 148, asked for its items, players and spawn point, took 1,870 ticks, and left
(`logs/world-8001.log`, `logs/capture-world-8001.jsonl`).

What that build was serving was the sniffer's head: `0x017C` at 20 bytes, `0x0186` at 4, `0x019A` at 32
(`logs/capture-world-8001.jsonl`, seq 1-4), where the reference and this tree's own wire send 6, 2 and 16
(`recordings/gwps/world/load-stream.log`, `recordings/asura/world/live-client-session-20261027-223528.jsonl`).
The name message was rewritten three times inside one hour - `u16` count and variable units (22 bytes), then
`u16` count padded to a fixed forty bytes (44), then `u32` count and variable units (24) - and no version of
it was checked against a wire capture, which shows the reference sending a `u16` count and that many units,
14 bytes for a five-character name.

The `0x8000` frames were read as a verdict and are not one: the last build's client acked `0x0186`, `148` and
`0x001E` (`logs/capture-world-8001.jsonl`, seq 13-18), while in the session of 2026-10-27 that loaded, walked
and travelled all 360 of its `0x8000` frames carry a zero payload and the client worked. Neither capture names
`0x017D` in an ack, working or not.

## What was actually wrong (written 2026-11-01, after the reversion)

**The attempt aimed at the wrong layer.** The sizes it moved ~30 messages to came from
`tools/sniff-capture.py`'s `entry.size`, and that is the client's message struct: a four-byte header, fields
at native width, zero padding, and in `0x000B` absolute pointers. The wire is the packed form - two-byte
header, the fields alone - and the server that works with this client writes it: `recordings/gwps/world/load-stream.log`
has `0x017C` 6 bytes, `0x019A` 16, `0x001E` 6, `0x017D` 14. This tree already sent those sizes, and a
session with them loaded, walked, chatted and travelled.

So the loop was closed on itself: the table was generated from the struct layer, every message was bent
towards it, and the run-time line "this size is not the reference's" fired on correct frames. The one
client-measured fact the attempt already had - an eight-byte `0x001E` answers `007`, a six-byte one plays -
says exactly what the reference's send log says, and was read as an exception instead of as the answer.

What survives from the attempt, and what it is evidence of:

- the client tells you where it stops, and one message per build is the only way to read that answer;
- the offline suites cannot see this class of defect, because the harness frames by this tree's own table;
- the instrumentation is sound and its reference was not: a capture of what this server sent
  (`--capture`, `logs/capture-world-8001.jsonl`) is wire evidence, and a capture of what the client held is
  not. That distinction is now the rule in `docs/03-protocol.md` 5e.

## Why it failed, in process terms

1. **I changed many messages per step and asked the client to judge a batch.** `AGENTS.md` says one change,
   one subject, and the client is the judge. Each of those is a different message in a different phase; a
   client failure after a batch cannot be attributed to one of them, which is exactly what happened.
2. **The offline checks cannot see this class of defect.** The test client frames the stream by our own
   packet table, so a message of the wrong size is internally consistent and the tests pass. `cargo test`
   was therefore never evidence about the client, and I reported it as if it were.
3. **The live-size table's reference was the wrong layer.** It was generated from the sniffer captures, so
   it enforced the client's struct sizes: it reported "every one at a size the reference uses" while every
   corrected message was 2 or more bytes longer than the wire, and the client was refusing the load.
4. **I invented a feature.** The NPC population was built because the gap list ranked it first, which is a
   decision about the product, not about the stream. It was not requested and it was not reviewed.

## What to take from the branch

```powershell
git log --oneline main..packet-parity-attempt          # one commit: 9818bf2
git diff main packet-parity-attempt -- <path>          # any single message, tool or document
git checkout packet-parity-attempt -- crates/asura-protocol/src/packets.rs   # a piece, without the rest
```

Worth taking first, in this order:

1. `crates/asura-protocol/src/live_sizes.rs` + `tools/gen-live-sizes.py` + the `send()` check in
   `crates/asura-world/src/connection.rs`: the run-time "this size is not the reference's" line, **re-pointed
   at the wire** - generated from `recordings/gwps/world/load-stream.log` and the `recordings/asura/`
   sessions, never from a sniffer capture. As the branch generates it, from the client's struct, it is not
   worth taking.
2. `tools/stream-diff.py`, `tools/stream-sizes.ps1`, `tools/client-check.ps1`: reading a session back.
3. ~~`crates/asura-world/src/capture.rs` wiring~~ - **already taken, and differently**: the session
   capture is on `main` and switched by the command line, `--capture PATH` or `ASURA_CAPTURE`
   (`crates/asura-world/src/main.rs:131-136`, used at `connection.rs:485,534`), not by
   `config/services.toml`.
4. The individual message corrections, one at a time, each verified with the client before the next.

Not worth taking: the NPC population, unless it is wanted as a feature in its own right.

## If this is attempted again

- One message per build, and the client in the loop before the next. No batch of sizes, no ordering change
  and a message change in the same build.
- Before any of that: work out which layer the number came from. A wire size is a size this server sent
  (`--capture`, `ASURA_TRACE`) or the reference sent (`recordings/gwps/world/load-stream.log`); a sniffer
  capture is a count, an order, a name and a trigger, and never a size (`docs/03-protocol.md` 5e).
- When the client stops, the answer is which message it refused, not which size is wrong in theory. The
  `0x8000` frames are not that answer: they are identical in a session that works.
- A counted field (a name, an item's modifiers, a blob) is settled by the wire, where the count is, never by
  a sniffer capture - `--raw` included. `0x017D` is 14 bytes for a five-character name
  (`recordings/gwps/world/load-stream.log`), and that is the whole of what this message needs.
- A message the client refuses tells you more than ten that it accepts, and the eight-byte `0x001E` that
  answered `007` was that fact all along.

## Delivered work, from the handoff

### What M2 delivered

| piece | where |
|---|---|
| lease ledger migration (`lease` table: one row per reserved slot, unique per session) | `crates/asura-db/migrations/0002_leases.sql` |
| typed placement queries - register, heartbeat, reserve, release, reap, reconcile, remove | `crates/asura-db/src/placement.rs` |
| synchronous, idempotent migrations and the schema tests that keep them honest | `crates/asura-db/src/lib.rs` |
| the concurrency acceptance test (capacity N-1, N concurrent reservations, exactly N-1 win; full-pool refusal; reaper; reconcile) | `crates/asura-db/tests/placement.rs` |
| the fake-world harness (`asura-fakeworld scenario`) | `crates/asura-fakeworld/` |
| one command: integration tests + harness scenario | `tools/m2-verify.ps1` |
| dependencies, added with the code that needs them: `sqlx` 0.8 (PostgreSQL), `tokio` 1, `uuid` | `Cargo.toml` |

The acceptance result, verbatim from a run: capacity 4 with 5 concurrent reservations -> 4 won, 1
refused; `leased` ends at capacity; the ledger matches; the reaper freed a silent world's leases and
left a fresh world's alone; reconcile rebuilt the counter from the ledger. Repeated 5x, no flakiness.

**Why a ledger beside the counter:** `world_pool.leased` makes the reservation atomic; the `lease`
rows make a lease *identifiable*, which is what the reaper needs to free a dead world's slots and the
reconciler needs to rebuild `leased` from the worlds instead of trusting the table
(`docs/history.md`, rule 2). They are inserted, released and reaped together in one
transaction, so `count(lease) <= leased <= capacity` holds at every commit and the
`world_pool_never_oversold` CHECK never fires.


### The ten-minute idle run (T13)

All three clauses of the reference's T13 are now measured by one command:

```powershell
cargo test -p asura-world --test load a_ten_minute_idle_pair -- --ignored --nocapture
```

It holds **two** connections idle for ten minutes, counts the ticks and pings, watches the world's peer
count, and samples the process's working set every fifteen seconds through
`K32GetProcessMemoryInfo` (kernel32, no crate). The verdict is asserted, not narrated: after a tenth of the
window to let the allocator settle, a working set that grows by more than 8 MiB fails the test.

Two details worth knowing before trusting a "flat" line. The sampler **checks itself first**: it allocates
16 MiB, writes one byte per page -- a zeroed allocation is demand-zero pages and does not move the working
set, which the first version of this check proved the hard way -- and fails if the sample does not see it.
And the window is `ASURA_SOAK_SECONDS` (default 600), so the mechanism can be exercised in a minute:

```powershell
$env:ASURA_SOAK_SECONDS = 45
cargo test -p asura-world --test load a_ten_minute_idle_pair -- --ignored --nocapture
```

The world runs as a task in the test process here, so the sample is everything the world allocated plus the
harness -- the reference runs a process per map and watches that process. Same question, one process fewer.


### The pieces the inventory added (M5)

| piece | where |
|---|---|
| `bag`, `item`, the item-id sequence, and `characters.pos_plane` | `crates/asura-db/migrations/0006_inventory.sql` |
| reading a character's bags and items, seeding the default set, finding the item a drop names | `crates/asura-db/src/inventory.rs` |
| the seed: the reference's own six bags and nine items, generated from the capture | `crates/asura-db/src/inventory/seed.rs` |
| the capture itself, kept as the fixture the rows must reproduce | `crates/asura-db/src/inventory.rs:243` (`default_inventory()`) |
| the generator for both halves: the seed rows and the captured bytes | `tools/gen-default-inventory.py`, retired with the parity tooling (`docs/16`); the two files above are its committed output |
| `CREATE_NAMED_ITEM`, `ITEM_MOVE_TO_LOCATION`, `ITEM_SET_PROFESSION` | `crates/asura-protocol/src/game/mod.rs` |
| the item block from rows, the bags in model order, the worn ids from the equipped bag | `crates/asura-world/src/item.rs:70` (`items_block`), `:120` (`inventory_block`), `crates/asura-world/src/player.rs:145` (`player_block`) |
| `DROP_ITEM`, finished rather than copied: the stack is taken down, and nothing goes on the wire | `crates/asura-world/src/item.rs:187` (`drop_item`), `:243-249` (why it sends nothing); dispatched at `dispatch.rs:73` |
| `ITEM_MOVE` and `EQUIP_ITEM`, which the reference does not dispatch at all | `crates/asura-world/src/item.rs` + `asura_db::inventory::{move_item, first_free_carrying_slot, take_from_stack}`, dispatched at `dispatch.rs:73-79` |
| the position saved on leaving and resumed on entering | `asura_db::game::record_position`/`position`, `Standing::resume` |
| the live-database proof: seeding, round trip, drop scoping, position | `crates/asura-db/tests/inventory.rs` |

### The pieces the map data added (D1-D3)

A map's geometry is one `Gw.dat` entry, compressed, and the reference reads it in three steps: the
archive's index, its decompressor, and `GmMapDataImport.c`'s chunk reader. All three are here.

| piece | where |
|---|---|
| the archive index: header, file table, TOC hash, one entry's bytes | `crates/asura-world/src/gwdat.rs` |
| `FaCompress.c`'s decoder: the bit window, the node tree, the block loop | `crates/asura-world/src/fa_compress.rs` |
| its six static tables | `crates/asura-world/src/fa_compress_tables.rs` |
| the container and the path graph: planes, trapezoids, x/y/sink nodes, portals | `crates/asura-world/src/map_data.rs` |
| "which trapezoid is this position in": `PathFindTrapezoid` | same file (`PathStaticData::trapezoid_at`) |
| every catalogue map imported out of a real `Gw.dat`, with its spawn point | `crates/asura-world/examples/map-census.rs` |
| the test that a dumped graph answers exactly the walks the `Gw.dat` import answers | `crates/asura-world/tests/path_dump.rs` |
| the A*: neighbour and portal visits, the heap, the waypoint builder | `crates/asura-world/src/pathfind.rs` |
| its fixed-point reciprocal and square-root tables | `crates/asura-world/src/path_tables.rs` |
| the in-module pathfinding tests: the visits, the heap, the waypoint builder | `crates/asura-world/src/pathfind.rs` |
| the graphs a world serves with, imported at startup from `Gw.dat` | `crates/asura-world/src/lib.rs` (`PathGraphs`), `src/main.rs` (`--gwdat`, `ASURA_GW_DAT`) |
| the walk itself: destination, direction, speed, waypoints, and the tick that advances them | `crates/asura-world/src/agent.rs` (`Motion`, `begin_walk`, `advance`) |
| `MOVE_TO_COORD`'s answer: waypoint 0, or the refusal that is a position broadcast | `crates/asura-world/src/lib.rs` (`0x803E`) |

The pathing proof is data-driven now: `tests/path_dump.rs` asserts a graph written with
`--dump-path-graphs` answers exactly the walks the `Gw.dat` import answers, and the in-module tests in
`src/pathfind.rs` pin the visits and the waypoint builder. They skip cleanly without a local `.dat`.

The parity/regeneration apparatus that used to sit beside them (`parity-pathing.py`, `path-oracle.c`,
`gen-path-cases.py`, the walk fixtures) was retired when parity closed (docs/16); the measurements it
produced are described in `docs/history.md`, item D, and are history. One of those
measurements is worth keeping as a note rather than a command: `PathBuildAddWaypointAndReduce` reads
its intersection parameters after `FindIntersectionPoint` has declined to write them (parallel lines),
so one of the catalogue cases had a different waypoint list on different runs. That was a property of
the reference, not of this port, and `path_dump.rs` is the guard that a waypoint list is stable now.

### The pieces the character settings added (B5)

The login reply's character list carries a 62-byte settings block per character, and this server used to send
the same captured one for every character. It is now built from the row.

| piece | where |
|---|---|
| the five columns the blob needs (level, campaign, is-pvp, secondary profession, helmet status) | `crates/asura-db/migrations/0009_character_settings.sql` |
| reading them back, plus the armour pieces derived from the equipped bag | `asura_db::game::settings` |
| `CharacterSettings`'s packing, byte for byte, and the reference's own blob kept as the expected output | `crates/asura-protocol/src/client/messages.rs` (`character_settings`, `CHARACTER_SETTINGS_EXTENDED`) |
| the login reply building it per character instead of sending the constant | `crates/asura-auth/src/lib.rs` |
| the creation defaults the reference writes (`level = 1`, `is_pvp` when the campaign is none, helmet shown) | `asura_db::inventory::create_character` |
| the proof: the built block equals the reference's, the seeded character's does not, and the row round-trips | `messages.rs` tests, `crates/asura-auth/tests/login.rs`, `crates/asura-db/tests/inventory.rs` |
| the schema diff, as a historical record: 95 reference columns, each stored, derived or explained | `docs/history.md` (history) |

Two schema details worth not re-learning: `smallint` and `integer` are *different Rust types* to sqlx, so a
query that reads a mixed set of them needs `::int` casts or it fails at runtime; and the settings block is
always 62 bytes even for a character wearing nothing, because the reference sends `sizeof(CharacterSettings)`
- the five piece slots are blank, not absent. The comparison tools that produced `docs/12` and the load
allowances were retired with the rest of the parity tooling (docs/16).

### The pieces M3 added

| piece | where |
|---|---|
| connection classification: the three version headers, their sizes, and the refusal to guess | `crates/asura-protocol/src/client/version.rs` |
| framing: header plus table-sized body, no length field, with encode-side size validation | `crates/asura-protocol/src/client/frame.rs` |
| the packet table: every header of both families, 100 auth rows and 682 game rows, with names, sizes and field layouts | `crates/asura-protocol/src/packets.rs` |
| the portal identity column, and why it is not the account id | `crates/asura-db/migrations/0003_portal_identity.sql` |
| the login decision: find the session, close the others, read the characters | `crates/asura-db/src/login.rs` |
| the refusal rules under test against the live database | `crates/asura-db/tests/login.rs` |
| the three identity pairs the rig's portal DLL presents, seeded idempotently | `tools/seed-dev.ps1` |
| one command for the whole login path, client not needed yet | `tools/m3-verify.ps1` |
| the database's record of its own migrations, and the refusal to run against a stale schema | `crates/asura-db/src/migrations.rs` (+ `tests/migrations.rs`) |
| ARC4, ported from `arc4.c`, checked against published vectors and round trips | `crates/asura-protocol/src/client/arc4.rs` |
| the key exchange: seeds, fixed-constant DH, and the protocol's own key derivation | `crates/asura-protocol/src/client/handshake.rs` |
| the login batch's fixed 591-byte settings blob | `crates/asura-protocol/src/client/login_fixtures.rs` |
| typed layouts for the messages the login path uses, with sizes checked against the table | `crates/asura-protocol/src/client/messages.rs` |
| a test client: the same protocol from the other end, so the service is testable without the game | `crates/asura-protocol/src/client/peer.rs` |
| the service itself: listener, key exchange, ARC4 both ways, login, trace | `crates/asura-auth/src/lib.rs` (+ `src/main.rs`) |
| a client anyone can run against a live service | `crates/asura-auth/examples/login.rs` |
| the end-to-end proof: a real socket, the real database, the character list | `crates/asura-auth/tests/login.rs` |

The tables and the fixture blobs carry `#[rustfmt::skip]` and are the implementation now. Their
generators and checks (the `gen-*` scripts, the `parity-*` scripts, the oracle binaries) were retired
when parity closed (docs/16); if a table ever disagrees with the client, the client is the test.

Two PowerShell traps are fixed in the scripts and worth not re-introducing:

- A native program's stderr (psql's `NOTICE`, cargo's progress) becomes an error record that
  `$ErrorActionPreference = "Stop"` treats as fatal, so repeated or idempotent runs aborted. Each
  script runs native programs with the preference relaxed and judges them by exit code alone; psql
  takes its options before the database URI.
- A `Get-Content`/`Set-Content` round trip on a UTF-8 markdown file reads it as ANSI and writes back a
  different encoding, which turned every em dash in this file into mojibake once already. The rule for
  that is `AGENTS.md` #5.

### Definition of done for the first session (met)

A `world` registered in the database, capacity published, reservations proven atomic under
concurrency, and the reaper proven to free a dead world's leases - all against the live database,
repeatedly, with no client, no protocol and no simulation. That is M2, and it is done.
