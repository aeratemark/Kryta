# The parity audit: every message, every subject, against ASURA

This is the ledger of Rule 0 (`AGENTS.md`). It lists **everything** ASURA has, what this tree has for
it, where, and a verdict. It is not a summary and it is not a sample: the message tables below are
ASURA's own protocol table read out row by row, and the subject tables are its own module tree read
directory by directory.

Verdicts: **ported** (the ASURA code translated, cited where used) - **ported thin** (ported, but only
part of what ASURA gives the same subject) - **ours** (written here, no ASURA counterpart: a defect of
Rule 0) - **missing** (ASURA has it, this tree has nothing) - **absent** (a subject with no code at
all here).

Measured 2026-11-12:

| | ASURA | this tree |
|---|---|---|
| the protocol table | `crates/asura-protocol/src/packets.rs`, **782 rows** | `game/src/Server/Protocol/Tables.rs`, 683 rows (the game connection's rows, copied) |
| client messages it handles | **36** (15 auth + 21 game) | 23 (12 auth + 11 game) |
| server messages it emits | **87** (8 auth + 79 game) | 71 |
| its own builders | `crates/asura-protocol/src/game/mod.rs`, **80** | `game/src/Server/Packets/`, 57 |
| its game program's modules | `crates/asura-world/src/`, **29 modules** | `game/src/`, 43 nouns, **10 with code** |
| its services | 12 crates | 2 programs + 3 tools |

## A. The services

| ASURA | what it is | this tree | verdict |
|---|---|---|---|
| `crates/asura-auth` | the login program | `authserver/` | ported thin (see B1, B2) |
| `crates/asura-world` | the game program's library | `game/` | ported thin (see C, D) |
| `crates/asura-worldserver` | the game program | `worldserver/` | ported thin |
| `crates/asura-protocol` | both connections' wire, one builder per message | `game/src/Server/Packets/`, `shared/src/Packets/`, `authserver/src/Protocol/` | ported thin (57 of 80 builders) |
| `crates/asura-store` | the database state: identity, sessions, characters, inventory, placement, catalogue | `game/src/Characters/`, `game/src/Accounts/`, `game/src/Maps/`, `game/src/DataStores/` | ported thin |
| `crates/asura-db` | migrations and their runner | `data/sql/base/`, `database/src/Updater/` | ported thin |
| `crates/asura-config` | every settings file it reads | `common/src/Configuration/`, `data/*.json`, `*/…json` | ported thin |
| `crates/asura-gateway` | the front door: a client's game connection routed to a world | `authserver/src/Server/front_door.rs` | ported |
| `crates/asura-webgate` | the portal's four HTTP endpoints | `game/src/Portal/` | ported |
| `crates/asura-rules` | the decisions that are neither transport nor I/O: placement, names | `game/src/Maps/Placement.rs` (one rule) | **ours**, thin: one of three placement rules |
| `crates/asura-supervisor` | the stack's one command | `tools/supervisor/` | ported |
| `crates/asura-fakeworld` | a test harness world | nothing | **missing** |

## B. The auth connection

### B1. Client -> server, 15 messages ASURA handles

| header | ASURA | this tree | verdict |
|---|---|---|---|
| `0x8000` | `AUTH_CMSG_HEARTBEAT` (6) | - | **missing** |
| `0x8001` | `AUTH_CMSG_SEND_COMPUTER_INFO` (138) | - | **missing** |
| `0x8002` | `AUTH_CMSG_SEND_COMPUTER_HASH` (22) | - | **missing** |
| `0x8007` | `AUTH_CMSG_DELETE_CHARACTER` (50) | - | **missing** |
| `0x8009` | `AUTH_CMSG_UPDATE_CHARACTER_SETTINGS` (118) | `session.rs` `UPDATE_CHARACTER_SETTINGS`, acknowledged | ported thin: the settings are not stored |
| `0x800A` | `AUTH_CMSG_CHANGE_PLAY_CHARACTER` (50) | `session.rs` `CHANGE_PLAY_CHARACTER` | ported |
| `0x800D` | `AUTH_CMSG_DISCONNECT` (6) | `session.rs` `DISCONNECT` | ported |
| `0x800E` | `AUTH_CMSG_SET_PLAYER_STATUS` (6) | `session.rs` `SET_PLAYER_STATUS` | ported |
| `0x801C` | `AUTH_CMSG_ADD_ACCESS_KEY` (62) | `session.rs` `ADD_ACCESS_KEY` | ported thin: acknowledged, not decoded |
| `0x8020` | `AUTH_CMSG_SETTING_UPDATE_CONTENT` (522) | `session.rs` `SETTING_UPDATE_CONTENT` | ported thin: acknowledged, not stored |
| `0x8021` | `AUTH_CMSG_SETTING_UPDATE_SIZE` (10) | `session.rs` `SETTING_UPDATE_SIZE` | ported thin: acknowledged |
| `0x8026` | `AUTH_CMSG_ACCEPT_EULA` (3) | `session.rs` `ACCEPT_EULA`, recorded on the account row | ported |
| `0x8029` | `AUTH_CMSG_REQUEST_GAME_INSTANCE` (26) | `session.rs` `answer_game_instance` | ported thin: no leases, no capacity, one placement rule of three |
| `0x8035` | `AUTH_CMSG_ASK_SERVER_RESPONSE` (6) | `session.rs` `ASK_SERVER_RESPONSE` | ported |
| `0x8038` | `AUTH_CMSG_PORTAL_ACCOUNT_LOGIN` (150) | `session.rs` `answer_login` | ported thin: the character in the message is not recorded on the session |
| `0x800B` | (its own district-count answer, `crates/asura-auth`) | `session.rs` `QUERY_INSTANCE_COUNT` | **ours**: fixed counts, not read from a world |

### B2. Server -> client, 8 messages ASURA emits

| header | ASURA | this tree | verdict |
|---|---|---|---|
| `0x0000` | `AUTH_SMSG_HEARTBEAT` | `login_reply::heartbeat`, sent by `greeting` | ported |
| `0x0001` | `AUTH_SMSG_SESSION_INFO` | `login_reply::session_info` | ported |
| `0x0003` | `AUTH_SMSG_REQUEST_RESPONSE` | `login_reply::request_response` | ported |
| `0x0007` | `AUTH_SMSG_CHARACTER_INFO` (138) | `login_reply::character_info` | ported |
| `0x0009` | `AUTH_SMSG_GAME_SERVER_INFO` (42) | `login_reply::game_server_info` | ported |
| `0x0011` | `AUTH_SMSG_ACCOUNT_INFO` (275) | `login_reply::account_info` | ported (the EULA byte was hardcoded 0 until 2026-11-12) |
| `0x0014` | `AUTH_SMSG_FRIEND_STREAM_END` | `login_reply::friend_stream_end` | ported |
| `0x0016` | `AUTH_SMSG_ACCOUNT_SETTINGS` (2058) | `login_reply::account_settings_frame`, from the settings file | ported |

## C. The game connection

### C1. Client -> server, 21 messages ASURA handles

| header | ASURA handler | this tree | verdict |
|---|---|---|---|
| `0x8008` | `dispatch.rs:86` `GAME_CMSG_DISCONNECT` | - | **missing** |
| `0x8009` | `dispatch.rs:119` `GAME_CMSG_PING_REPLY` (discarded) | - | **missing** (logs it as unanswered) |
| `0x800C` | `dispatch.rs:122` `GAME_CMSG_HEARTBEAT` (discarded) | - | **missing** |
| `0x800D` | `dispatch.rs:36` `GAME_CMSG_PING_REQUEST` | `WorldSession` `GAME_PING = 0x800D`, echoed | **ours**: the echo is not ASURA's answer |
| `0x8028` | `agent.rs:1062` `handle_cancel_movement` | - | **missing** |
| `0x802C` | `item.rs:187` `handle_drop_item` | `CLIENT_ITEM_DESTROY = 0x8069` | **ours**: the header is not the drop's |
| `0x8030` | `item.rs` `handle_equip_item` | `CLIENT_EQUIP_ITEM` | ported thin |
| `0x803D` | `agent.rs:887` `handle_position_update` | read, the point stored, nothing sent | **ours** |
| `0x803E` | `agent.rs:710` `handle_move_to_coord` | read, the point stored, nothing sent | **ours** |
| `0x8047` | `agent.rs:1117` `handle_last_position` | read, nothing | **ours** |
| `0x804F` | `dispatch.rs:114` `handle_unequip_item` | - | **missing** |
| `0x8060` | `creation.rs:275` `handle_change_profession` | `WorldSession` `CHAR_CREATION_CHANGE_PROF` | ported |
| `0x8064` | `chat.rs` `handle_chat_message` | - | **missing** |
| `0x8072` | `item.rs` `handle_item_move` | `CLIENT_ITEM_MOVE` | ported thin |
| `0x8088` | `agent.rs:1219` `handle_request_spawn` | `CLIENT_LOAD_REQUEST_SPAWN` | ported thin |
| `0x8089` | `creation.rs:190` `handle_request_player` | `CHAR_CREATION_REQUEST_PLAYER` | ported |
| `0x808A` | `creation.rs:259` `handle_request_armors` | `CHAR_CREATION_REQUEST_ARMORS` | ported |
| `0x808B` | `creation.rs:346` `handle_confirm` | `confirm_creation` | ported |
| `0x8090` | `player.rs:430` `handle_request_players` | the players answer | ported |
| `0x8091` | `item.rs:448` `handle_request_items` | `CLIENT_LOAD_REQUEST_ITEMS` | ported thin |
| `0x80B1` | `travel.rs` `handle_party_travel` | `CLIENT_PARTY_TRAVEL = **0x00B1**` | **ours**: wrong header, and the answer is not sent |

**Also in ASURA's dispatch and absent here:** the travel announcement (`handle_travel_init`), the
doorway test after every accepted move (`travel::portal_crossed`), and the character of the session's
own selection.

### C2. Server -> client, 79 messages ASURA emits

| header | what it is | this tree | verdict |
|---|---|---|---|
| `0x000C` | ping request | `ping_request` | ported |
| `0x000D` | ping reply | `ping_reply` | ported |
| `0x000F` | account feature | `account_feature` | ported |
| `0x0018` | PvP heroes | `pvp_update_unlocked_heroes` | ported |
| `0x001A` | PvP item add | `pvp_item_stream_add` | ported |
| `0x001B` | PvP item stream end | `pvp_item_stream_end` | ported |
| `0x001D` | PvP skills | `pvp_update_unlocked_skills` | ported |
| `0x001E` | the simulation tick | `world_simulation_tick` | ported (the value was a counter until 2026-11-12) |
| `0x001F` | load time | `world_update_load_time` | ported |
| `0x0020` | create agent | `world_create_agent` | ported |
| `0x0021` | remove agent | - | **missing** |
| `0x0022` | controlled agent | `world_update_controlled_agent` | ported |
| `0x0025` | agent update direction | - | **missing** |
| `0x0028` | agent stop moving | - | **missing** |
| `0x0029` | agent move to point | - | **missing** |
| `0x002B` | agent update speed | - | **missing** |
| `0x002C` | agent update position | - | **missing** |
| `0x0030` | character update info | `character_update_info` | ported thin: fixed values, no name |
| `0x0037` | attribute points | `agent_update_attribute_points` | ported |
| `0x003A` | attributes | `agent_update_attributes` | ported |
| `0x003C` | player flags | `player_update_flags` | ported |
| `0x0048` | display cape | `agent_display_cape` | ported |
| `0x0059` | player create | `player_create` | ported |
| `0x005A` | player destroy | - | **missing** |
| `0x005D` | chat, the body | - | **missing** |
| `0x005E` | chat, the server speaking | - | **missing** |
| `0x0061` | chat, which player spoke | - | **missing** |
| `0x006E` | visual equipment | `update_agent_visual_equipment` | ported |
| `0x0070` | hard mode | `hard_mode_unlocked` | ported |
| `0x008A` | cartography data | - | **missing** |
| `0x008B` | map init start | - | **missing** |
| `0x0094` | unlocked lists | `map_update_unlocked_list` | ported |
| `0x0099` | map update current | `map_update_current` | ported |
| `0x009C` | morale | `agent_update_morale` | ported |
| `0x009F` | agent property, int | `agent_property_update_int` | ported |
| `0x00A2` | agent property, float | `agent_property_update_float` | ported |
| `0x00B0` | a player's party size | - | **missing** |
| `0x00B1` | a player's party | - | **missing** |
| `0x00B6` | unlocked professions | `player_update_unlocked_professions` | ported |
| `0x00B7` | profession | `player_update_profession` | ported |
| `0x00DA` | skillbar | `skillbar_update` | ported thin: eight empty slots only |
| `0x00DB` | unlocked skills | `update_unlocked_skills` | ported thin: always empty |
| `0x00E9` | the progress record | `character_update_factions` | ported |
| `0x00EA`-`0x00ED` | the four faction caps | `faction_max` | ported |
| `0x00EF` | visibility flags | `char_visibility_flags` | ported |
| `0x00F0` | initial effects | `agent_initial_effects` | ported |
| `0x00F2` | instance loaded, the team token | `instance_loaded` | ported |
| `0x013E` | item moved to a location | `item_move_to_location` | ported |
| `0x013F` | inventory create bag | `inventory_create_bag` | ported |
| `0x0141` | gold | `update_gold_storage` | ported |
| `0x0144` | item stream create | `item_stream_create` | ported |
| `0x0147` | weapon set | `item_weapon_set` | ported |
| `0x0148` | active weapon set | `item_set_active_weapon_set` | ported |
| `0x014D` | item remove | `item_remove` | ported |
| `0x015A` | item set profession | `item_set_profession` | ported |
| `0x015F` | create unnamed item | - | **missing** |
| `0x0161` | create named item | `create_named_item` | ported |
| `0x017C` | instance load head | `instance_load_head` | ported thin: the two unlock bytes are a constant |
| `0x017D` | the player's name | `instance_load_player_name` | ported |
| `0x0186` | player-data start | `instance_player_data_start` | ported |
| `0x0188` | creation success | `char_creation_success` | ported |
| `0x0189` | creation start | `char_creation_start` | ported |
| `0x018A` | player-data done | `instance_player_data_done` | ported |
| `0x018B` | creation error | `char_creation_error` | ported |
| `0x018E` | the load is over | `instance_load_finish` | ported |
| `0x0196` | the entry position | `instance_load_spawn_point` | ported |
| `0x0198` | manifest done | `instance_manifest_done` | ported thin: the words are the trace's, not derived |
| `0x0199` | manifest phase | `instance_manifest_phase` | ported |
| `0x019A` | instance load info | `instance_load_info` | ported |
| `0x01A0` | the instance table | - | **missing** |
| `0x01A6` | transfer game server info | - | **missing** |
| `0x01AC` | ready for a spawn | `ready_for_map_spawn` | ported |
| `0x01B3` | which party the character is in | `character_update_party` | ported |
| `0x01CC` | party player add | `party_player_add` | ported |
| `0x01D3` | party create | `party_create` | ported |
| `0x01D4` | party member stream end | `party_member_stream_end` | ported |

Missing from the emitted set: **16 messages** - `0x0021`, `0x0025`, `0x0028`, `0x0029`, `0x002B`,
`0x002C`, `0x005A`, `0x005D`, `0x005E`, `0x0061`, `0x008A`, `0x008B`, `0x00B0`, `0x00B1`, `0x015F`,
`0x01A0`, `0x01A6`.

## D. ASURA's game modules, one by one

| ASURA module | what it holds | this tree | verdict |
|---|---|---|---|
| `agent.rs` (66 KB) | `AgentState`: position, plane, facing, motion, waypoints, speed, move type, the sync copy; `handle_move_to_coord`, `handle_position_update`, `handle_cancel_movement`, `handle_last_position`, `handle_request_spawn`; the tolerance `50 + speed * elapsed` (max elapsed 2 s); `densify`; the live cadence 145 units | `game/src/Agents/mod.rs`, 54 lines: an id, x, y, facing and `moved_to` | **ours** |
| `pathfind.rs` (67 KB) | the walker: graphs, portals, `Walk::Found/NoPath/NoGraph` | nothing | **missing** |
| `path_tables.rs`, `graph_dump.rs` | the pack's path data and its dump tool | nothing | **missing** |
| `map_data.rs` (49 KB) | the map blob out of `Gw.dat`: districts, spawns, portals, the pmap | nothing (`game/Pack/` is 85 bytes) | **missing** |
| `gwdat.rs`, `fa_compress.rs`, `fa_compress_tables.rs` | reading `Gw.dat`: the archive index and its compression | nothing | **missing** |
| `map.rs`, `map_spawns.rs`, `spawns.rs`, `catalogue.rs` | the map as a definition, its spawn points, the campaign spawns, the catalogue | `game/src/Maps/` (27 KB), `map` + `map_spawn` rows | ported thin: one map, one spawn point, no arrivals |
| `player.rs` (27 KB) | `player_block` (34 frames), the player's own state, the unlocked bitmaps | `WorldSession`'s players answer | ported |
| `creation.rs` (21 KB) | the four creation handlers, the equipment table, the confirm | `game/src/Creation/mod.rs`, `WorldSession`'s creation arms | ported |
| `item.rs` (23 KB) | `items_block`, `handle_request_items`, drop, move, equip, unequip, the equipped ids | `game/src/Server/Packets/ItemPackets.rs`, `WorldSession`'s item arms | ported thin: no unequip, no drop item, no ground item |
| `instance.rs` (21 KB) | the instance: slots, the clock, `introduction`, `farewell`, the tick | `game/src/Districts/` (23 KB), `WorldSession`'s tick | ported thin: no broadcast to other players |
| `instance_table.rs` | `0x01A0`, the instances a client may travel to | nothing | **missing** |
| `travel.rs` (26 KB) | the transfer: reserve, hand-off, the redirect (`0x01A6`, `0x0099`), party travel, doors | `game/src/Maps/Travels.rs`, `authserver`'s travel row | ported thin: no redirect, no doors |
| `portals.rs` (20 KB) | portal crossing, the pack's portal rows | nothing | **missing** |
| `travel_targets.rs` | where a named travel object leads | nothing | **missing** |
| `cartography.rs` | the exploration grid: `0x008B`, `0x008A`, nothing explored | nothing | **missing** |
| `chat.rs` | the body/commit split, channels, the sender's name | `game/src/Chat/` (95 bytes) | **missing** |
| `connection.rs` (26 KB) | the connection's own loop, the ping, the heartbeat, the load's order | `game/src/Server/WorldSession.rs` | ported thin |
| `dispatch.rs` | the 21 messages above | `WorldSession::answer` | **ours** where it differs (see C1) |
| `world.rs` | the world: pools, the map catalogue, the instance registry | `game/src/World/` (20 KB) | ported thin |
| `replay.rs`, `capture.rs` | replaying a captured block, capturing this server's | nothing | **missing** |
| `harness_off.rs` | the test harness switch | nothing | **missing** |
| `agent_defaults.rs`, `character_defaults.rs` | the numbers a body and a character are created with | `WorldConfig`'s agent settings | ported |
| `pvp_items.rs` | the 387 unlockable PvP items | `game/src/DataStores/PvpItems.rs` | ported (copied) |
| `default_equipment.rs` (94 KB) | what each campaign and profession starts wearing | `data/equipment.json` | ported (imported) |

## E. This tree's nouns with nothing in them

Forty-three nouns exist; **ten have code** (Accounts, Agents, Characters, Creation, DataStores,
Districts, Maps, Portal, Server, World) and **thirty-three are a doc comment and nothing else**:
Achievements, Addons, Behaviour, Cache, Calendar, Chat, Cinematics, Combat, Conditions, Drops, Events,
FileCheck, Globals, Guilds, Handlers, Heroes, Misc, Modules, Motd, Movement, Pack, Parties, Pools,
Quests, Reputation, Scripting, Skills, Support, Terrain, Texts, Time, Tools, Trader. Some are library
placeholders with no ASURA counterpart by design (Achievements, Calendar, Reputation, Support,
Trader); the rest are subjects ASURA implements in the module named in D and this tree has not ported.

## F. Behaviours and settings that carry a rule

| rule | ASURA | this tree | verdict |
|---|---|---|---|
| a walk's cadence: a destination every ~145 units | `agent.rs:625` `LIVE_CADENCE_STEP`, `live_cadence_step()` | nothing | **missing** |
| the position tolerance: `50 + speed * elapsed`, elapsed capped at 2 s | `agent.rs:59-63` | nothing | **missing** |
| where a travel request is placed: three rules | `asura-rules/src/lib.rs`, `placement` | `game/src/Maps/Placement.rs`, rules 1 and 2 | ported thin |
| a world's capacity, leases, health | `asura-store/src/placement.rs` | nothing | **missing** |
| the EULA version in the account info | `asura-auth/src/lib.rs:610` | read from the account row (2026-11-12) | ported |
| a name is acceptable | `asura-rules/src/lib.rs`, `name_is_acceptable` | nothing: the confirm writes whatever name it is sent, and only the unique constraint refuses | **missing** |
| the character's default numbers: level, gold, appearance, secondary profession | `config/World/Vanilla/character_defaults.json` | `WorldConfig`, `confirm_creation` | ported thin |
| the agent's numbers: health, energy, regeneration, speed | `config/World/Vanilla/agent_defaults.json` | `WorldConfig` agent settings | ported |
| the pack's own settings files | `config/World/<pack>/…` (maps, spawns, portals, cartography) | `data/realms.json`, `data/sql/`, `worldserver.json` | ported thin |

## G. What this means, in one sentence per row

- **Movement does not exist here.** Six of ASURA's frames are missing, both its handlers that walk are
  missing, its path graph and its cadence are missing, and what this tree has is a point that the
  client's reports overwrite.
- **Chat does not exist here.** Three frames, one handler, absent.
- **The map's content does not exist here.** `Gw.dat` is unread, so no agents, no props, no path
  graph, no portals, no map view.
- **The travel path is a fragment.** No redirect, no instance table, no doors, no capacity.
- **Sixteen emitted messages and eight handled client messages are absent**, and two handled headers
  are wrong (`0x00B1` for the party travel, `0x8069` for the drop).
- **Everything the two client runs exercised is ported**, which is exactly why only those two runs'
  faults appeared: the port had never gone past them.

## H. The order Rule 0 imposes

1. **Movement, whole** - `agent.rs`'s state and its four handlers, the five frames and their `_live`
   forms, the cadence, the tolerance, and `pathfind.rs` with the pack's path graph.
2. **The wrong headers and the absent client messages** - `0x80B1`, `0x802C`, `0x8028`, `0x804F`,
   `0x8008`/`0x8009`/`0x800C`, and the auth program's `0x8000`/`0x8001`/`0x8002`/`0x8007`.
3. **The absent frames on the load path** - `0x0021`, `0x005A`, `0x00B0`/`0x00B1`, `0x015F`,
   `0x01A0`, `0x01A6`, `0x008A`/`0x008B`.
4. **Chat**, whole: three frames, one handler, the channel rules.
5. **The map's content** - `gwdat.rs`, `fa_compress*.rs`, `map_data.rs`, `path_tables.rs`,
   `portals.rs`, `travel_targets.rs` - because everything in 1 and 4 needs it.
6. **The travel path**, whole: leases, capacity, the redirect, the instance table, doors.
7. **The rules layer**, whole: placement's three rules, `name_is_acceptable`, the rest.
8. The remaining modules in D, in their own order.
