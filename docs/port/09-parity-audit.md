# The protocol audit: every packet, both directions, against ASURA

The ledger of Rule 0 (`AGENTS.md`). **Scope: the protocol and the packets only** - the wire, the
messages, their fields, their builders, their handlers and the order they go in. This tree's own
structure is not audited here and does not need to be: it is AzerothCore's.

Every number below was measured on 2026-11-12 by reading ASURA's table and this tree's tables with
the same script and diffing them row by row (`target/diff-tables.py`, `target/diff-headers.py`; the
scripts are scratch and are not committed, the findings are).

Verdicts: **ported** (ASURA's bytes and behaviour, translated, cited) - **ported thin** (ported, but
only part of what ASURA gives the same message) - **ours** (written here, no ASURA counterpart: a
defect of Rule 0) - **missing** (ASURA has it, this tree sends or handles nothing).

## 1. The packet table

| | ASURA | this tree |
|---|---|---|
| the game connection | 682 rows | 682 rows |
| the auth connection | 100 rows | 100 rows |
| rows in ASURA's table and not here | - | **0** |
| rows here and not in ASURA's table | - | **0** |
| rows whose `size`, `name` and field list differ | - | **0** of the 742 compared |

The table is copied whole and it agrees: 782 rows, the same headers, the same declared sizes, the
same names, the same field kinds and lengths. Forty of the 782 rows are written with an evidence
string my comparison script cannot read as fields (`0x0026`, `0x0035`, `0x003C`, `0x0067`, `0x006F`,
`0x007C`, `0x007D`, `0x008A`, `0x008B`, `0x008C`, `0x008D`, `0x009C`, `0x00A7`, `0x00B5`, `0x00CF`,
`0x00D0`, `0x00EF`, `0x0107`, `0x0115`, `0x011B`, `0x011C`, `0x011F`, `0x012E`, `0x012F`, `0x015B`,
`0x015E`, `0x0168`, `0x016E`, `0x0194`, `0x0196`, `0x01A0`, `0x01AE`, `0x01BE`, `0x01BF`, `0x800A`,
`0x800B`, `0x8092`, `0x8093`, `0x80B1`, `0x80C1`); those are compared by hand only, and three of them
(`0x009C`, `0x015E`, `0x0196`) were checked by hand during the loads.

**The field vocabulary is the same fifteen kinds** (`MsgHeader`, `MsgHeaderGame`, `AgentId`, `Float`,
`Vect2`, `Vect3`, `Byte`, `Word`, `Dword`, `Blob`, `String16`, `Array8`, `Array16`, `Array32`,
`NestedStruct`), and the walker that turns a row into a frame length is the same walk
(`shared/src/Packets/Frame.rs` from `ASURA/crates/asura-protocol/src/client/frame.rs`).

## 2. The wire beneath the messages

| subject | ASURA | this tree | verdict |
|---|---|---|---|
| the frame: no length field, the row decides | `client/frame.rs` (30 KB) | `shared/src/Packets/Frame.rs` (11 KB) | ported |
| a counted field: `u16` count then that many units, capacity bounds and never pads | `frame.rs` `Shape::Counted` | same | ported |
| a nested struct: a `u8` count and that many repeats | `frame.rs` `Shape::Nested` | same | ported |
| `MsgHeaderGame`: four bytes on the wire | `frame.rs` | same | ported |
| the 21-byte file check and its 32-byte answer | `client/filecheck.rs`, `config/Server/file_check.json` | `authserver/src/Server/file_check.rs`, constants | ported |
| the seed exchange and the key derivation | `client/handshake.rs` (20 KB) | `shared/src/Cryptography/Handshake.rs` (15 KB) | ported thin: one derivation path only |
| the cipher | `client/arc4.rs` | `shared/src/Cryptography/AuthCrypt.rs` | ported |
| the 64-byte version message | `client/game_version.rs` (8.9 KB) | `game/src/Server/Protocol/mod.rs` | ported |
| the connection's first four bytes | `client/version.rs` (11 KB) | `authserver/src/Server/stream.rs`, `front_door.rs` | ported thin |
| the reference client (a test double that speaks the wire) | `client/reference_client.rs` (17 KB) | nothing | **missing**: no test proves a whole conversation against an independent speaker |
| the trace of every byte | `client/trace.rs` | `shared/src/network` trace + logs | ported thin |

## 3. Client -> server: the 36 messages ASURA handles

### 3.1 The auth connection (15)

| header | message | ASURA handler | this tree | verdict |
|---|---|---|---|---|
| `0x8000` | `AUTH_CMSG_HEARTBEAT` | `asura-auth/src/lib.rs`, `handle_heartbeat` | - | **missing** |
| `0x8001` | `AUTH_CMSG_SEND_COMPUTER_INFO` | `lib.rs`, `decode_computer_info` (`client/messages.rs:251`) | - | **missing** |
| `0x8002` | `AUTH_CMSG_SEND_COMPUTER_HASH` | `lib.rs` | - | **missing** |
| `0x8007` | `AUTH_CMSG_DELETE_CHARACTER` | `lib.rs` | - | **missing** |
| `0x8009` | `AUTH_CMSG_UPDATE_CHARACTER_SETTINGS` | `lib.rs:507` | `session.rs` acknowledged | ported thin: not stored |
| `0x800A` | `AUTH_CMSG_CHANGE_PLAY_CHARACTER` | `lib.rs`, `decode_change_character` | `session.rs`, recorded on the session | ported |
| `0x800D` | `AUTH_CMSG_DISCONNECT` | `lib.rs` | `session.rs` | ported |
| `0x800E` | `AUTH_CMSG_SET_PLAYER_STATUS` | `lib.rs` | `session.rs` | ported |
| `0x801C` | `AUTH_CMSG_ADD_ACCESS_KEY` | `lib.rs` | `session.rs` | ported thin: not decoded |
| `0x8020` | `AUTH_CMSG_SETTING_UPDATE_CONTENT` (522) | `lib.rs` | `session.rs` | ported thin: not stored |
| `0x8021` | `AUTH_CMSG_SETTING_UPDATE_SIZE` | `lib.rs` | `session.rs` | ported thin: not stored |
| `0x8026` | `AUTH_CMSG_ACCEPT_EULA` | `lib.rs:487`, `decode_eula` | `session.rs`, recorded | ported |
| `0x8029` | `AUTH_CMSG_REQUEST_GAME_INSTANCE` | `lib.rs:507`, `travel::answer` | `session.rs` `answer_game_instance` | ported thin: no lease, no capacity, no world probe, 1 of 3 placement rules |
| `0x8035` | `AUTH_CMSG_ASK_SERVER_RESPONSE` | `lib.rs` | `session.rs` | ported |
| `0x8038` | `AUTH_CMSG_PORTAL_ACCOUNT_LOGIN` | `lib.rs`, `decode` | `session.rs` `answer_login` | ported thin: the named character is not recorded |

### 3.2 The game connection (21)

| header | message | ASURA handler | this tree | verdict |
|---|---|---|---|---|
| `0x8008` | `GAME_CMSG_DISCONNECT` | `dispatch.rs:86` | - | **missing** |
| `0x8009` | `GAME_CMSG_PING_REPLY` | `dispatch.rs:119`, discarded | - | **missing** |
| `0x800C` | `GAME_CMSG_HEARTBEAT` | `dispatch.rs:122`, discarded | - | **missing** |
| `0x800D` | `GAME_CMSG_PING_REQUEST` | `connection::handle_ping_request` | echoed back | **ours**: the answer is not the ping reply's |
| `0x8028` | `GAME_CMSG_CANCEL_MOVEMENT` | `agent.rs:1062` | - | **missing** |
| `0x802C` | `GAME_CMSG_DROP_ITEM` (7 bytes) | `item::handle_drop_item` | listened for on `0x8069` | **ours**: wrong header |
| `0x8030` | `GAME_CMSG_EQUIP_ITEM` | `item::handle_equip_item` | handled | ported thin |
| `0x803D` | `GAME_CMSG_POSITION_UPDATE` (26) | `agent.rs:887` | read, point stored, nothing sent | **ours** |
| `0x803E` | `GAME_CMSG_MOVE_TO_COORD` (14) | `agent.rs:710` | read, point stored, nothing sent | **ours** |
| `0x8047` | `GAME_CMSG_LAST_POS_BEFORE_MOVE_CANCELED` | `agent.rs:1117` | read, nothing | **ours** |
| `0x804F` | `GAME_CMSG_UNEQUIP_ITEM` (6) | `dispatch.rs:114` | - | **missing** |
| `0x8060` | `GAME_CMSG_CHAR_CREATION_CHANGE_PROF` (4) | `creation.rs:275` | handled | ported |
| `0x8064` | `GAME_CMSG_SEND_CHAT_MESSAGE` (286) | `chat::handle_chat_message` | - | **missing** |
| `0x8072` | `GAME_CMSG_ITEM_MOVE` (9) | `item::handle_item_move` | handled | ported thin |
| `0x8088` | `GAME_CMSG_INSTANCE_LOAD_REQUEST_SPAWN` | `agent.rs:1219` | handled | ported thin |
| `0x8089` | `GAME_CMSG_CHAR_CREATION_REQUEST_PLAYER` (2) | `creation.rs:190` | handled | ported |
| `0x808A` | `GAME_CMSG_CHAR_CREATION_REQUEST_ARMORS` (2) | `creation.rs:259` | handled | ported |
| `0x808B` | `GAME_CMSG_CHAR_CREATION_CONFIRM` (58) | `creation.rs:346` | handled | ported |
| `0x8090` | `GAME_CMSG_INSTANCE_LOAD_REQUEST_PLAYERS` (19) | `player.rs:430` | handled | ported |
| `0x8091` | `GAME_CMSG_INSTANCE_LOAD_REQUEST_ITEMS` (4) | `item.rs:448` | handled | ported thin |
| `0x80B1` | `GAME_CMSG_PARTY_TRAVEL` (9) | `travel::handle_party_travel` | listened for on `0x00B1` | **ours**: wrong header, no answer |

**Score: 20 ported or ported thin (11 auth + 9 game), 6 ours, 10 missing.**

## 4. Server -> client: the 87 messages ASURA emits

### 4.1 The auth connection (8)

| header | message | ASURA builder | this tree | verdict |
|---|---|---|---|---|
| `0x0000` | `AUTH_SMSG_HEARTBEAT` | `client/messages.rs:540` | `login_reply::heartbeat` | ported |
| `0x0001` | `AUTH_SMSG_SESSION_INFO` | `messages.rs:532` | `login_reply::session_info` | ported |
| `0x0003` | `AUTH_SMSG_REQUEST_RESPONSE` | `RequestResponse::frame` | `login_reply::request_response` | ported |
| `0x0007` | `AUTH_SMSG_CHARACTER_INFO` (138) | `messages.rs:498` | `login_reply::character_info` | ported |
| `0x0009` | `AUTH_SMSG_GAME_SERVER_INFO` (42) | `messages.rs:690`, `host_for` | `login_reply::game_server_info`, `host_blob` | ported |
| `0x0011` | `AUTH_SMSG_ACCOUNT_INFO` (275) | `messages.rs:593` | `login_reply::account_info` | ported (the EULA byte was ours until 2026-11-12) |
| `0x0014` | `AUTH_SMSG_FRIEND_STREAM_END` | `messages.rs:524` | `login_reply::friend_stream_end` | ported |
| `0x0016` | `AUTH_SMSG_ACCOUNT_SETTINGS` (2058) | `messages.rs:572` | `login_reply::account_settings_frame` | ported |

### 4.2 The game connection (79)

| header | message | ASURA builder | this tree | verdict |
|---|---|---|---|---|
| `0x000C` | ping request | `ping_request` | `misc_packets::ping_request` | ported |
| `0x000D` | ping reply | `ping_reply` | `misc_packets::ping_reply` | ported |
| `0x000F` | account feature | `account_feature` | `misc_packets::account_feature` | ported |
| `0x0018` | PvP heroes | `pvp_update_unlocked_heroes` | same | ported |
| `0x001A` | PvP item add | `pvp_item_stream_add` | same | ported |
| `0x001B` | PvP item stream end | `pvp_item_stream_end` | same | ported |
| `0x001D` | PvP skills | `pvp_update_unlocked_skills` | same | ported |
| `0x001E` | simulation tick | `world_simulation_tick(delta_ms)` | same | ported (ours until 2026-11-12: a counter) |
| `0x001F` | load time | `world_update_load_time` | same | ported |
| `0x0020` | create agent (99) | `world_create_agent`, `CreateAgent` | same | ported |
| `0x0021` | remove agent | `world_remove_agent` | - | **missing** |
| `0x0022` | controlled agent | `world_update_controlled_agent` | same | ported |
| `0x0025` | agent update direction (15) | `agent_update_direction` / `_live` | - | **missing** |
| `0x0028` | agent stop moving | `agent_stop_moving` | - | **missing** |
| `0x0029` | agent move to point (18) | `agent_move_to_point` / `_live` | - | **missing** |
| `0x002B` | agent update speed (11) | `agent_update_speed` / `_live` | - | **missing** |
| `0x002C` | agent update position (16) | `agent_update_position` | - | **missing** |
| `0x0030` | character update info (94) | `character_update_info(name)` | fixed words, no name | ported thin |
| `0x0037` | attribute points | `agent_update_attribute_points` | same | ported |
| `0x003A` | attributes (202) | `agent_update_attributes` | same | ported |
| `0x003C` | player flags | `player_update_flags` | same | ported |
| `0x0048` | display cape | `agent_display_cape` | same | ported |
| `0x0059` | player create (91) | `player_create` | same | ported |
| `0x005A` | player destroy | `player_destroy` | - | **missing** |
| `0x005D` | chat: the body | `chat_message_core` | - | **missing** |
| `0x005E` | chat: the server speaking | `chat_message_server` | - | **missing** |
| `0x0061` | chat: which player spoke | `chat_message_local` | - | **missing** |
| `0x006E` | visual equipment | `update_agent_visual_equipment` | same | ported |
| `0x0070` | hard mode | `hard_mode_unlocked` | same | ported |
| `0x008A` | cartography data (262) | `cartography_data` | - | **missing** |
| `0x008B` | map init start (14) | `map_init_start` | - | **missing** |
| `0x0094` | unlocked lists (662) | `map_update_unlocked_list` | same | ported |
| `0x0099` | map update current | `map_update_current` | same | ported |
| `0x009C` | morale | `agent_update_morale` | same | ported |
| `0x009F` | agent property, int | `agent_property_update_int` | same | ported |
| `0x00A2` | agent property, float | `agent_property_update_float` | same | ported |
| `0x00B0` | a player's party size | `player_update_party_size` | - | **missing** |
| `0x00B1` | a player's party | `player_update_party` | - | **missing** |
| `0x00B6` | unlocked professions | `player_update_unlocked_professions` | same | ported |
| `0x00B7` | profession | `player_update_profession` | same | ported |
| `0x00DA` | skillbar (79) | `skillbar_update` | eight empty slots only | ported thin |
| `0x00DB` | unlocked skills (518) | `update_unlocked_skills` | always empty | ported thin |
| `0x00E9` | the progress record (62) | `character_update_factions` | same | ported |
| `0x00EA`-`0x00ED` | the four faction caps | `faction_max` | same | ported |
| `0x00EF` | visibility flags | `char_visibility_flags` | same | ported |
| `0x00F0` | initial effects | `agent_initial_effects` | same | ported |
| `0x00F2` | instance loaded | `instance_loaded` | same | ported |
| `0x013E` | item moved to a location | `item_move_to_location` | same | ported |
| `0x013F` | inventory create bag | `inventory_create_bag` | same | ported |
| `0x0141` | gold | `update_gold_storage` | same | ported |
| `0x0144` | item stream create | `item_stream_create` | same | ported |
| `0x0147` | weapon set | `item_weapon_set` | same | ported |
| `0x0148` | active weapon set | `item_set_active_weapon_set` | same | ported |
| `0x014D` | item remove | `item_remove` | same | ported |
| `0x015A` | item set profession | `item_set_profession` | same | ported |
| `0x015F` | create unnamed item (25) | `create_unnamed_item` | - | **missing** |
| `0x0161` | create named item | `create_named_item` | same | ported |
| `0x017C` | instance load head | `instance_load_head(pve, pvp)` | two unlock bytes are a constant | ported thin |
| `0x017D` | the player's name | `instance_load_player_name` | same | ported |
| `0x0186` | player-data start | `instance_player_data_start` | same | ported |
| `0x0188` | creation success (1092) | `char_creation_success` | same | ported |
| `0x0189` | creation start | `char_creation_start` | same | ported |
| `0x018A` | player-data done | `instance_player_data_done` | same | ported |
| `0x018B` | creation error | `char_creation_error` | same | ported |
| `0x018E` | the load is over | `instance_load_finish` | same | ported |
| `0x0196` | the entry position (26) | `instance_load_spawn_point` | same | ported |
| `0x0198` | manifest done | `instance_manifest_done(phase, map_id)` | three fixed words | ported thin |
| `0x0199` | manifest phase | `instance_manifest_phase` | same | ported |
| `0x019A` | instance load info | `instance_load_info` | same | ported |
| `0x01A0` | the instance table (102) | `instance_table` | - | **missing** |
| `0x01A6` | transfer game server info (46) | `transfer_game_server_info` | - | **missing** |
| `0x01AC` | ready for a spawn | `ready_for_map_spawn` | same | ported |
| `0x01B3` | which party the character is in | `character_update_party` | same | ported |
| `0x01CC` | party player add | `party_player_add` | same | ported |
| `0x01D3` | party create | `party_create` | same | ported |
| `0x01D4` | party member stream end | `party_member_stream_end` | same | ported |

**Score: 62 ported or ported thin, 17 missing, 0 ours among the emitted set.**

### 4.3 Builders ASURA has that no message above covers

`map_update_current_redirect` (`0x0099` in its four-byte-header form), `map_init_start` and
`cartography_data` (counted above), `create_unnamed_item`, `agent_update_effects` (`0x00F0`'s
update form), `agent_update_direction`/`_speed`/`_position` and their `_live` forms, and
`equipped_slot` (the item-kind to slot table, which this tree has as
`creation::equipped_slot`). The `_live` forms exist because the live client's structs differ from the
sniffer's; **this tree has neither of the movement families**, so it has neither the choice nor the
setting that selects one.

## 5. The sequences - the protocol is also the order

| sequence | ASURA | this tree | verdict |
|---|---|---|---|
| the greeting before the login | `session_info` then `heartbeat` | same | ported |
| the login answer | one `character_info` per character, `account_settings`, `friend_stream_end`, `account_info`, `request_response` (`asura-auth/src/lib.rs:669-746`) | same five, same order | ported |
| the creation conversation | `0x8089` -> the player-data block; `0x808A` -> the PvP half; `0x8060` -> removals, `0x00DB`, `0x00B7`, the new set; `0x808B` -> `0x0188` + `0x0099` (`creation.rs:190-519`) | same | ported |
| the items ask | the stream, the bags and items, four weapon sets, the gold, four faction caps, the hard-mode flag, the character summary, the map, ready-for-spawn, the manifest (`item.rs:70`) | same, plus the reference's own `0x0196`-before-manifest order | ported thin: no `0x015E` |
| the spawn ask | the entry point and the tick | the entry point and the tick | ported thin |
| the players ask | 34 frames, in one order (`player.rs:382-388` for the party) | the same 34, the same order | ported |
| the travel | the redirect `0x01A6`, `0x0099`, `0x00B0`, `0x00B1`, `0x005A`, `0x0021` (`travel.rs`) | none: the travel request is read and logged | **ours** |
| a player joining a map others are in | `introduction` (`0x0059`, `0x00B0`, `0x00B1`, `0x0020`) and `farewell` (`0x005A`, `0x0021`) | nothing | **missing** |
| chat | `0x005D` bodies, one of `0x005E`/`0x0061` committing | nothing | **missing** |
| the exploration grid | `0x008B` then `0x008A`, 0.58-0.73 s into the load, once per instance (`cartography.rs`) | nothing | **missing** |

## 6. What the protocol audit says, in numbers

| | count |
|---|---|
| messages ASURA handles (client -> server) | 36 |
| ... this tree handles as ASURA does | **20** (11 auth, 9 game) |
| ... this tree handles, but not as ASURA does | **6** |
| ... of those, on the wrong header | 2 (`0x80B1` listened for as `0x00B1`; `0x802C` as `0x8069`) |
| ... of those, read and dropped | 3 (`0x803D`, `0x803E`, `0x8047`) |
| ... of those, answered differently | 1 (`0x800D`: echoed instead of answered with the ping reply) |
| ... this tree does not handle | **10** |
| messages ASURA emits (server -> client) | 87 |
| ... this tree emits | **70** (8 auth, 62 game) |
| ... this tree does not emit | **17** |
| messages this tree emits that ASURA does not have | 1: the district-count answer on the auth connection |
| sequences ported whole | 6 of 9 |
| the packet table | identical, 782 rows (682 game + 100 auth) |

## 7. The defects, in the order the client feels them

1. **Movement is not a port.** Five frames (`0x0025`, `0x0028`, `0x0029`, `0x002B`, `0x002C`) and
   both `_live` forms are missing, and the three client reports are read and dropped. The client walks;
   the world says nothing; the tick was a counter until 2026-11-12. This is the largest single break
   and the one the owner has seen.
2. **The party travel is on the wrong header** (`0x00B1` instead of `0x80B1`), so the message the
   client sends when it uses the world map is not recognised at all, and nothing is answered.
3. **The drop item is on the wrong header** (`0x8069` instead of `0x802C`).
4. **Chat is absent**: three emitted frames and one handled message.
5. **The cartography pair is absent**, so the map view asserts.
6. **The travel sequence is absent**: `0x01A6`, `0x00B0`/`0x00B1`, `0x005A`, `0x0021`, `0x01A0`.
7. **`0x015F` (unnamed item) and `0x804F` (unequip) are absent.**
8. **Six auth messages are absent** (`0x8000`, `0x8001`, `0x8002`, `0x8007`, and the two settings
   uploads are acknowledged rather than stored).
9. **The thin rows**: `0x0030` with no name, `0x00DA`/`0x00DB` always empty, `0x017C`'s unlock bytes,
   `0x0198`'s words, `0x009F`'s three properties, the items block without `0x015E`.

## 8. The order this audit imposes

1. The movement family, both forms: `0x0025`, `0x0028`, `0x0029`, `0x002B`, `0x002C`,
   `agent_move_to_point_live`, `agent_update_direction_live`, `agent_update_speed_live`,
   `agent_stop_moving`, `agent_update_position`, and the handlers behind them (`handle_move_to_coord`,
   `handle_position_update`, `handle_cancel_movement`, `handle_last_position`).
2. The two wrong headers and the six absent client messages.
3. The travel sequence and the introduction/farewell pair.
4. Chat, whole.
5. The cartography pair.
6. `0x015F`, `0x804F`, `0x01A0`.
7. The thin rows, each replaced by ASURA's own.
