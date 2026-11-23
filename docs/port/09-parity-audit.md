# The protocol audit: every packet, both directions, against ASURA

The ledger of Rule 0 (`AGENTS.md`). **Scope: the protocol and the packets only** - the wire, the
messages, their fields, their builders, their handlers and the order they go in. This tree's own
structure is not audited here and does not need to be: it is AzerothCore's.

When another research project appears to show additional behavior, use the packet-sequence
completeness criterion in `docs/research-sources.md` before adding a subject here. The source's
adapter, architecture, or client build does not disqualify its packet evidence; the question is whether
it completes a packet contract Kryta does not. A larger table or an isolated packet builder is not, by
itself, a missing Kryta function.

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

**Re-synced 2026-11-22 against ASURA `071735e`** (the newer checkout this tree now ports party and cartography from): of its 682 game rows, six had changed since `ca9f532` - `0x0021` and `0x005A` (8 -> 6 bytes), `0x005E` and `0x0061` (10 -> 5), `0x00B0` (12 -> 5) and `0x00B1` (12 -> 6), each a struct-layer reading ASURA corrected against a live client - and those six rows are now ASURA's. Twelve rows gained a name in ASURA and no size or field change; they are not copied yet. The table is copied whole and it agrees: 782 rows, the same headers, the same declared sizes, the
same names, the same field kinds and lengths. **That sentence was false until round 20**: this tree
carried 683 game rows against ASURA's 682, and the extra was `0x00B1` read as a client-to-server row
named `GAME_CMSG_PARTY_TRAVEL` - the very mistake the code comments record ("this tree listened for
`0x00B1`, which is the *server's* party message, until 2026-11-12"). A header-for-header comparison of
the two tables found it; the row is gone, one session test that had been written around it now sends
the client's real frame (`B1 80`, nine bytes), and the two tables now agree exactly. Forty of the 782 rows are written with an evidence
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

## 3. Client -> server: the 37 messages ASURA handles

### 3.1 The auth connection (15)

| header | message | ASURA handler | this tree | verdict |
|---|---|---|---|---|
| `0x8000` | `AUTH_CMSG_HEARTBEAT` | `lib.rs:307-312` | `HEARTBEAT`: `heartbeat(16)` | ported |
| `0x8001` | `AUTH_CMSG_SEND_COMPUTER_INFO` | `lib.rs:250-254` | `COMPUTER_INFO`: recorded, answered with nothing | ported |
| `0x8002` | `AUTH_CMSG_SEND_COMPUTER_HASH` | `lib.rs:256-275` | `COMPUTER_HASH`: the session info, with the salt already on the wire | ported |
| `0x8007` | `AUTH_CMSG_DELETE_CHARACTER` | `lib.rs:409-474` | `DELETE_CHARACTER`: by name on the account, status 0 / 23 / 2 | ported |
| `0x8009` | `AUTH_CMSG_UPDATE_CHARACTER_SETTINGS` | `lib.rs:369-382` | `session.rs`: acknowledged with status 0 | ported |
| `0x800A` | `AUTH_CMSG_CHANGE_PLAY_CHARACTER` | `lib.rs`, `decode_change_character` | `session.rs`, recorded on the session | ported |
| `0x800D` | `AUTH_CMSG_DISCONNECT` | `lib.rs` | `session.rs` | ported |
| `0x800E` | `AUTH_CMSG_SET_PLAYER_STATUS` | `lib.rs` | `session.rs` | ported |
| `0x801C` | `AUTH_CMSG_ADD_ACCESS_KEY` | `lib.rs:384-406` | `session.rs`: the units counted, status 0, nothing kept | ported |
| `0x8020` | `AUTH_CMSG_SETTING_UPDATE_CONTENT` (522) | `lib.rs:426-489`, `messages.rs:584-601` (ASURA `071735e`; `ca9f532` only acknowledged it) | `session.rs` `SETTING_UPDATE_CONTENT`: chunks of the open upload appended, the completed blob stored on `account.settings`, status 0 | ported |
| `0x8021` | `AUTH_CMSG_SETTING_UPDATE_SIZE` | `lib.rs:382-424`, `messages.rs:579-582` (ASURA `071735e`; `ca9f532` only acknowledged it) | `session.rs` `SETTING_UPDATE_SIZE`: opens an upload of 1..=2048 bytes, status 0 | ported |
| `0x8026` | `AUTH_CMSG_ACCEPT_EULA` | `lib.rs:487`, `decode_eula` | `session.rs`, recorded | ported |
| `0x8029` | `AUTH_CMSG_REQUEST_GAME_INSTANCE` | `lib.rs:507`, `travel::answer` | `session.rs` `answer_game_instance` | ported thin: no lease, no capacity, no world probe, and 2 of `asura-rules`' 3 placement rules - see the stop below |
| `0x8035` | `AUTH_CMSG_ASK_SERVER_RESPONSE` | `lib.rs` | `session.rs` | ported |
| `0x8038` | `AUTH_CMSG_PORTAL_ACCOUNT_LOGIN` | `lib.rs:572-680` | `session.rs` `answer_login`: the named character is recorded, or the account's first | ported |

### 3.2 The game connection (22)

| header | message | ASURA handler | this tree | verdict |
|---|---|---|---|---|
| `0x8008` | `GAME_CMSG_DISCONNECT` | `dispatch.rs:86` | ends the connection | ported |
| `0x8009` | `GAME_CMSG_PING_REPLY` | `dispatch.rs:119`, discarded | discarded | ported |
| `0x800C` | `GAME_CMSG_HEARTBEAT` | `dispatch.rs:122`, discarded | discarded | ported |
| `0x800D` | `GAME_CMSG_PING_REQUEST` | `connection.rs:579-587`, `handle_ping_request` | `0x000D` and `World.PingValue` | ported |
| `0x8028` | `GAME_CMSG_CANCEL_MOVEMENT` | `agent.rs:1062` | `answer_cancel_movement` | ported |
| `0x802C` | `GAME_CMSG_DROP_ITEM` (7 bytes) | `item.rs:187`, `handle_drop_item` | `CLIENT_DROP_ITEM`: the stack is taken and nothing is sent | **ported thin**: the current ASURA's ground record/agent and the rules' `Op::Take` are not carried (7h) |
| `0x8030` | `GAME_CMSG_EQUIP_ITEM` | `item.rs:314`, `equip_item` | `answer_equip_item`, through `Op::Equip` | ported: `0x014B` and the worn slot's `0x006F` |
| `0x803D` | `GAME_CMSG_POSITION_UPDATE` (26) | `agent.rs:887`, then `dispatch.rs:44-54` | `answer_position_update`, then `cross_portal` | ported (the doorway its step crosses is judged since round 18) |
| `0x803E` | `GAME_CMSG_MOVE_TO_COORD` (14) | `agent.rs:710` | `answer_move_to_coord` | ported |
| `0x8047` | `GAME_CMSG_LAST_POS_BEFORE_MOVE_CANCELED` | `agent.rs:1117`, then `dispatch.rs:60-68` | `answer_last_position`, then `cross_portal` | ported (a taken correction crosses a doorway like a step) |
| `0x804F` | `GAME_CMSG_UNEQUIP_ITEM` (6) | `item.rs:555`, `handle_unequip_item` | `answer_unequip_item`, through `Op::Unequip` | ported: `0x014B` and the cleared `0x006F` (the current ASURA's `item.rs:784`) |
| `0x8060` | `GAME_CMSG_CHAR_CREATION_CHANGE_PROF` (4) | `creation.rs:275` | handled | ported: the body is `chapter, profession`, chapter `0` for a PvP character and `1`-`3` for Prophecies, Factions, Nightfall (`docs/game-rules.md` E226); the client sends it on entry, on every campaign change and on every profession change, and follows each one with seven `0x8084` |
| `0x8064` | `GAME_CMSG_SEND_CHAT_MESSAGE` (286) | `chat.rs:193`, `handle_chat_message` | `answer_chat_message` | ported |
| `0x8072` | `GAME_CMSG_ITEM_MOVE` (9) | `item.rs:255`, `move_item` | `answer_item_move`, through `Op::Move` | ported: `0x014B` per change, the bulk `0x006E` when a drag changes what is worn, and a refusal answers where the item really is (the current ASURA's `item.rs:704`) |
| `0x8088` | `GAME_CMSG_INSTANCE_LOAD_REQUEST_SPAWN` | `agent.rs:1219`, `map.rs:13-20` | `load_map`, the entry position | ported: one `0x0196` frame, which is `spawn_block` whole (the answer used to append a first `0x001E` of its own; the ticker sends the ticks) |
| `0x8089` | `GAME_CMSG_CHAR_CREATION_REQUEST_PLAYER` (2) | `creation.rs:190` | handled | ported |
| `0x808A` | `GAME_CMSG_CHAR_CREATION_REQUEST_ARMORS` (2) | `creation.rs:259` | handled | ported |
| `0x808B` | `GAME_CMSG_CHAR_CREATION_CONFIRM` (58) | `creation.rs:346` | handled | **ported thin**: the counted name and the appearance word are read and written (the row writer is shared with the management panel, while starting inventory and unlock initialization remain in the handler); the next dword (8 bytes of which only the first 4 are the appearance's unused half) and the trailing `createFlags` - Reforged `0x1`, Dhuum `0x2`, Melandru `0x4` (`docs/game-rules.md` E222-E223) - are read by neither this tree nor ASURA, and no column stores them |
| `0x8090` | `GAME_CMSG_INSTANCE_LOAD_REQUEST_PLAYERS` (19) | `player.rs:430` | handled | ported |
| `0x8091` | `GAME_CMSG_INSTANCE_LOAD_REQUEST_ITEMS` (4) | `item.rs:448` | handled | ported: ASURA sends no `0x015E` either |
| `0x80B1` | `GAME_CMSG_PARTY_TRAVEL` (9) | `travel.rs:279`, `handle_party_travel` | `answer_party_travel` and `begin_transfer` | ported |
| `0x80C1` | `GAME_CMSG_TRAVEL_INIT` (10) | `travel.rs:147`, `handle_travel_init` (`dispatch.rs:73`) | `answer_travel_init` | ported - **missing until round 17**, and it starts no transfer in either tree |

**Score: 37 ported or ported thin (15 auth + 22 game), 0 ours, 0 missing.**

## 4. Server -> client: the 87 messages ASURA emits

### 4.1 The auth connection (8)

| header | message | ASURA builder | this tree | verdict |
|---|---|---|---|---|
| `0x0000` | `AUTH_SMSG_HEARTBEAT` | `client/messages.rs:540` | `login_reply::heartbeat` | ported |
| `0x0001` | `AUTH_SMSG_SESSION_INFO` | `messages.rs:532` | `login_reply::session_info` | ported |
| `0x0003` | `AUTH_SMSG_REQUEST_RESPONSE` | `RequestResponse::frame` | `login_reply::request_response` | ported |
| `0x0007` | `AUTH_SMSG_CHARACTER_INFO` (138) | `messages.rs:498` | `login_reply::character_info` | ported (the settings block's armour pieces were empty until 2026-11-23, so every character was drawn naked at character select; they are now the equipped bag's, as `asura-db/src/game.rs:344-356` derives them) |
| `0x0009` | `AUTH_SMSG_GAME_SERVER_INFO` (42) | `messages.rs:690`, `host_for` | `login_reply::game_server_info`, `host_blob` | ported |
| `0x0011` | `AUTH_SMSG_ACCOUNT_INFO` (275) | `messages.rs:593` | `login_reply::account_info` | ported (the EULA byte was ours until 2026-11-12) |
| `0x0014` | `AUTH_SMSG_FRIEND_STREAM_END` | `messages.rs:524` | `login_reply::friend_stream_end` | ported |
| `0x0016` | `AUTH_SMSG_ACCOUNT_SETTINGS` (2058) | `messages.rs:572`; the blob chosen at `lib.rs:736-757`, `846-876` (ASURA `071735e`) | `login_reply::account_settings_frame`, carrying the account's stored upload, or the committed 591-byte block when it has none (`docs/evidence/ui-settings-persistence.md`) | ported |

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
| `0x0020` | create agent (99) | `world_create_agent`, `CreateAgent`, `CreateAgent::LIVING` (`game/mod.rs:1426-1555`) | same; `h000b`, `h001e`, `h0023`, `h0027` are the caller's (a player's are `LIVING`'s, unchanged bytes); an NPC's model word is `CHAR_CLASS_NPC_BASE` over its definition index | ported (the four per-body fields were constants until 2026-11-22) |
| `0x0027` | agent update speed base (10) | `agent_update_speed_base` (`game/mod.rs:2274-2281`) | `agent_packets::agent_update_speed_base`; sent by the NPC `Pace` message, which nothing emits until a body has a `walk_speed` | ported (the builder; see 7g on `walk_speed`) |
| `0x0021` | remove agent | `world_remove_agent` (`game/mod.rs:2181-2186`, ASURA `071735e`: six bytes) | `agent_packets::world_remove_agent` | ported (eight bytes, the struct layer, until 2026-11-22) |
| `0x0022` | controlled agent | `world_update_controlled_agent` | same | ported |
| `0x0025` | agent update direction (15) | `agent_update_direction` / `_live` | `agent_packets::agent_update_direction`, both forms | ported |
| `0x0028` | agent stop moving | `agent_stop_moving` | `agent_packets::agent_stop_moving` | ported |
| `0x0029` | agent move to point (18) | `agent_move_to_point` / `_live` | `agent_packets::agent_move_to_point`, both forms | ported |
| `0x002B` | agent update speed (11) | `agent_update_speed` / `_live` | `agent_packets::agent_update_speed`, both forms | ported |
| `0x002C` | agent update position (16) | `agent_update_position` | `agent_packets::agent_update_position` | ported |
| `0x0030` | character update info (94) | `character_update_info(name)` | `character_packets::character_update_info("")` | ported |
| `0x0037` | attribute points | `agent_update_attribute_points` | same | ported |
| `0x003A` | attributes (202) | `agent_update_attributes` | same | ported |
| `0x003C` | player flags | `player_update_flags` | same | ported |
| `0x0048` | display cape | `agent_display_cape` | same | ported |
| `0x0056` | NPC update properties (38 for a 4-unit name) | `npc_update_properties`, `NpcProperties` (`game/mod.rs:1596-1640`) | `agent_packets::npc_update_properties`, byte-tested against seq 229 | ported; not yet sent (7g, N3) |
| `0x0057` | NPC update model | `npc_update_model` (`game/mod.rs:1642-1650`) | `agent_packets::npc_update_model`, byte-tested against seq 230 | ported; not yet sent (7g, N3) |
| `0x0059` | player create (91) | `player_create` | same | ported |
| `0x005A` | player destroy | `player_destroy` (`game/mod.rs:2173-2179`, ASURA `071735e`: six bytes) | `character_packets::player_destroy` | ported (eight bytes, the struct layer, until 2026-11-22) |
| `0x005D` | chat: the body, one fragment | `chat_message_core` | `chat_packets::chat_message_core` over `all_chat_body`, fragmented at 121 | ported: the body is a **coded string** (`0x0108 0x0107 text 0x0001`, `GmText.c:43-57` and `Rurik/studies/chat/FINDINGS.md:263`) and long text is fragmented, not truncated. ASURA's table and this tree before 0012 wrote bare text capped at 122, which is what left `/maps` past one frame |
| `0x005E` | chat: the server speaking | `chat_message_server` | `chat_packets::chat_message_server(player, channel)` | ported: `[u16 player, u8 channel]`, **5 bytes** (`msgdefs.info` `GAME_SMSG_0094`; bytes `5e00 0100 06`, `FINDINGS.md:264`). ASURA wrote two dwords; **it is sent now** (`answer_line`, `Channel_Global`) as `GmChat.c:116-143` sends it, and the owner's live client read `/maps`'s answer off the screen - the first authored text rendered on this channel in either server |
| `0x0061` | chat: which player spoke | `chat_message_local` | `chat_packets::chat_message_local(player, channel)` | ported: `[u16 player, u8 channel]`, **5 bytes** (`msgdefs.info` `GAME_SMSG_0097`; bytes `6100 0100 03`). ASURA wrote two dwords |
| `0x006D` | NPC update weapons | `npc_update_weapons` (`game/mod.rs:1670-1679`) | `agent_packets::npc_update_weapons`, byte-tested against seq 251 | ported; not yet sent (7g, N3) |
| `0x006E` | visual equipment | `update_agent_visual_equipment` | same | ported |
| `0x0070` | hard mode | `hard_mode_unlocked` | same, and the byte is the character's campaign completion (`prophecies_finished`, `factions_finished`, `nightfall_finished`), not a map or a hardcoded 1 | ported |
| `0x008A` | cartography data (262) | `cartography_data` | `instance_packets::cartography_data` | ported |
| `0x008B` | map init start (14) | `map_init_start` | `instance_packets::map_init_start` | ported |
| `0x0094` | unlocked lists (662) | `map_update_unlocked_list` | same | ported |
| `0x0099` | map update current | `map_update_current` and its redirect form | both forms | ported |
| `0x009A` | agent update scale | `agent_update_scale` (`game/mod.rs:1661-1668`) | `agent_packets::agent_update_scale`, byte-tested against seq 514 | ported; not yet sent (7g, N3) |
| `0x009B` | agent update NPC name | `agent_update_npc_name` (`game/mod.rs:1652-1659`) | `agent_packets::agent_update_npc_name`, byte-tested against seq 231 | ported; not yet sent (7g, N3) |
| `0x009C` | morale | `agent_update_morale` | same | ported |
| `0x009F` | agent property, int | `agent_property_update_int` | same | ported |
| `0x00A2` | agent property, float | `agent_property_update_float` | same | ported |
| `0x00B0` | a player's party size | `player_update_party_size` (`game/mod.rs:2146-2161`, ASURA `071735e`: five bytes) | `party_packets::player_update_party_size` | ported (twelve bytes until 2026-11-22: a live client read the rest as `0x0000` and dropped with error 7) |
| `0x00B1` | a player's party | `player_update_party` (`game/mod.rs:2163-2171`, ASURA `071735e`: six bytes) | `party_packets::player_update_party` | ported (twelve bytes until 2026-11-22) |
| `0x00B6` | unlocked professions | `player_update_unlocked_professions` | same | ported |
| `0x00B7` | profession | `player_update_profession` | same | ported |
| `0x00DA` | skillbar (79) | `skillbar_update` | eight empty slots and eight zero masks, as ASURA sends them | ported |
| `0x00DB` | unlocked skills (518) | `update_unlocked_skills` | empty, as ASURA sends it | ported |
| `0x00E9` | the progress record (62) | `character_update_factions` | same | ported |
| `0x00EA`-`0x00ED` | the four faction caps | `faction_max` | same | ported |
| `0x00EF` | visibility flags | `char_visibility_flags` | same | ported |
| `0x00F0` | initial effects | `agent_initial_effects` | same | ported |
| `0x00F2` | instance loaded | `instance_loaded` | same | ported |
| `0x013E` | item moved to a location | `item_move_to_location` | same | ported |
| `0x013F` | inventory create bag | `inventory_create_bag` | same | ported |
| `0x0141` | gold | `update_gold_storage` | same | ported; since 7j the load sends the carried gold as `0x0140` and the stored gold as `0x0141`, each only when not 0 (`ASURA/crates/asura-world/src/item.rs:72-85`) - this tree had sent the carried gold as `0x0141`, which the client shows as storage gold |
| `0x0144` | item stream create | `item_stream_create` | same | ported |
| `0x0147` | weapon set | `item_weapon_set` | same | ported |
| `0x0148` | active weapon set | `item_set_active_weapon_set` | same | ported |
| `0x014D` | item remove | `item_remove` | same | ported |
| `0x015A` | item set profession | `item_set_profession` | same | ported |
| `0x015F` | create unnamed item (25) | `create_unnamed_item` | `item_packets::create_unnamed_item` | ported |
| `0x0161` | create named item | `create_named_item` | same | ported |
| `0x017C` | instance load head | `instance_load_head(pve, pvp)` | `(0x3F, 0x3F)` - ASURA's own default | ported |
| `0x017D` | the player's name | `instance_load_player_name` | same | ported |
| `0x0186` | player-data start | `instance_player_data_start` | same | ported |
| `0x0188` | creation success (1092) | `char_creation_success` | same | ported: the u16 after the name is a **map id** - the capture carries the creation instance `416` there, and two lineages name the field `mapId`; OpenTyria's `0x2211` is its own filler (`docs/game-rules.md` E231) |
| `0x0189` | creation start | `char_creation_start` | same | ported |
| `0x018A` | player-data done | `instance_player_data_done` | same | ported |
| `0x018B` | creation error | `char_creation_error` | same | **ported thin**: the body is a `u32` code, and the code this tree sends is `0x0106` (262), which appears in no reading of the client's own create-error table - 25 "already have a character with the name", 29 "already in use", 31 "not permitted" per the binary read, 39 per a second client-table read (`docs/game-rules.md` E230) - so a refused name shows the client's generic network error instead of the name message |
| `0x018E` | the load is over | `instance_load_finish` | same | ported |
| `0x0196` | the entry position (26) | `instance_load_spawn_point` | same | ported |
| `0x0198` | manifest done | `instance_manifest_done(phase, map_id)` | the 877 sentinel then the map, as ASURA sends them | ported |
| `0x0199` | manifest phase | `instance_manifest_phase` | same | ported |
| `0x019A` | instance load info | `instance_load_info` | same, and its agent is the id drawn **before** the load head, so it always matches the `0x0020` the load creates | ported |
| `0x01A0` | the instance table (102) | `instance_table` | `instance_packets::instance_table` | ported: ASURA's shape, and the content is every row with an `instance_value` - 137 pairs in ten batches |
| `0x01AC` | ready for a spawn | `ready_for_map_spawn` | same | ported |
| `0x01B3` | which party the character is in | `character_update_party` | same | ported |
| `0x01CC` | party player add | `party_player_add` | same | ported |
| `0x01D3` | party create | `party_create` | same | ported |
| `0x01D4` | party member stream end | `party_member_stream_end` | same | ported |

**Score: 79 ported or ported thin, 0 missing, 0 ours among the emitted set.**

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
| the creation conversation | `0x8089` -> the player-data block; `0x808A` -> the PvP half; `0x8060` -> removals, `0x00DB`, `0x00B7`, the new set; `0x808B` -> `0x0188` + `0x0099` (`creation.rs:190-519`) | same | ported, with one message neither tree answers: after each `0x8060` the client sends seven `0x8084` (`{u8 equipSlot, u8 colour}`, slots `4, 2, 3, 6, 5, 0, 1`) carrying the armour colours it picked, and both trees ignore them - the created armour keeps the colours the equipment data names (`docs/game-rules.md` E226-E227) |
| the items ask | the stream, the bags and items, four weapon sets, the gold, four faction caps, the hard-mode flag, the character summary, the map, ready-for-spawn, the manifest (`item.rs:70`) | same, plus the reference's own `0x0196`-before-manifest order | ported |
| the spawn ask | the entry point (`agent.rs:1219`, `map.rs:13-20`) | the same one frame | ported |
| the players ask | 34 frames, in one order (`player.rs:382-388` for the party) | the same 34 in order, with the title definitions during load and the test display selection afterward | ported for the ASURA block |
| title definitions and selection (`0x00F3`, `0x00F6`, `0x8058`, `0x8059`, `0x00F4`) | `asura-protocol/src/packets.rs:422-423,771-774` defines the messages; ASURA has no title handler | The Northlands capture places the title block after progress and flags and before `0x018E` (seq 534-582). Kryta sends current and next ranks before their track and omits deprecated rows 11 and 12. Retail selection returns a single displayed rank (`login-unfiltered-9292.jsonl`, seq 659-775). Hide clears it with rank zero (Preforged `packets-1360-15821281.jsonl:36252`). The loaded district distributes this value to peers. | **ours**, partial. The test states, selection, and clear are sent. Earning and persistence are outside this slice. [Title research](19-title-research.md) |
| custom title text (`GET /kryta/custom-titles.json`) | ASURA `asura-webgate/src/lib.rs:136-195` has the stock login routes | The patched client fetches public custom text for rows numbered 48 and above from the same world-pack catalog that supplies the gameplay title records. The stock login and title packet layouts stay the same. | **ours**, approved custom-client extension. [Title research](19-title-research.md) |
| the travel | the redirect `0x01A6`, `0x0099` in its twelve-byte form, `0x00B0`, `0x00B1`, `0x005A`, `0x0021` (`travel.rs:480-518`) | the same eight, in the same order, out of `begin_transfer` | ported: the crossing starts it from a step (`cross_portal`) and hands its landing over; the seat reservation is 7.4's named difference |
| a player joining a map others are in | `introduction` (`0x0059`, the guild block, `0x00B0`, `0x00B1`, `0x00A6`, `0x009F` property 36, the title/`0x003C`, `0x00F0`, `0x0020`, one `0x015E` per worn item, `0x006E`, `0x0048`) and `farewell` (`0x005A`, `0x0021`) (`instance.rs:1031-1124`, `player.rs:784-851`) | the same, after the newcomer's own load block, out of `game/Districts/Peers.rs`; a guilded peer's `0x0121` and agent property 30 follow its `0x0059` (7e) | ported |
| chat | `0x005D` body plus `0x0061` who spoke, and every generated answer printed (`chat.rs:193-353`) | `0x005D`* then `0x0061`, broadcast to the map and district, and every generated answer **sent** as `0x005D`* + `0x005E` on `Channel_Global` | ported with one corrected divergence: ASURA prints the answers and its two tags are eight body bytes where the wire has three. The reference sends them (`GmChat.c:116-143`), the client renders the pair (`Rurik/studies/chat/FINDINGS.md:263-268`), and both frames go to the asker rather than the body to everyone plus a tag to one (`0012-chat-text-is-sent.md`) |
| the exploration grid | `0x008B` then `0x008A`, 0.58-0.73 s into the load, once per instance (`cartography.rs`, `map.rs:22-48`) | the same two, right after the instance's record, with every band empty (`maps::cartography::init_frames`) | ported |
| the instance table | `0x01A0` batches of fifteen pairs, in the same phase (`instance_table.rs`, `map.rs:49-58`) | the same batches, from every row with a value (`maps::instance_table::table_for`) | ported |

## 6. What the protocol audit says, in numbers

| | count |
|---|---|
| messages ASURA handles (client -> server) | 36 |
| ... this tree handles as ASURA does | **36** (15 auth, 21 game) |
| ... this tree handles, but not as ASURA does | **0** |
| ... of those, answered differently | 0 |
| ... this tree does not handle | **0** |
| messages ASURA emits (server -> client) | 87 |
| ... this tree emits | **87** (8 auth, 79 game) |
| ... this tree does not emit | **0** |
| messages this tree emits that ASURA does not have | 1: the district-count answer on the auth connection |
| sequences ported whole | **11 of 11**, none ported thin and none missing |
| the packet table | identical, 782 rows (682 game + 100 auth) |
| `ASURA/crates/asura-world/src` | 36 files | all 36 have a verdict in 9: 29 ported (code or data), 4 this tree's arrangement, 3 ASURA's own harness |

These counts are measured against ASURA `ca9f532`. The party subject (7c) comes from the newer `071735e`
and is beyond them: it adds six requests handled and eighteen builders, and its verdict is in 7c.

## 7. The defects, in the order the client feels them

1. **The movement family walks on ASURA's own graphs, imported at start-up.** The five frames and their
   `_live` forms, `AgentState`, `densify`, the cadence, the tolerance and the four handlers are
   `ASURA`'s; the tick carries elapsed time. The **archive is ported** (`game/Pack/`): the index
   (`Gwdat.rs`) turns a file id into an entry - this machine's `Gw.dat` holds 156,315 of them - and the
   decoder (`FaCompress.rs`) turns a compressed entry into bytes, proved against three real map blobs
   that decode to exactly the size they announce and start with the RIFF signature `ffna`. The blob's
   parse (`Terrain/MapData.rs`, `map_data.rs`), the dumped form (`Terrain/GraphDump.rs`,
   `graph_dump.rs`), the table-driven arithmetic (`Terrain/PathTables.rs`: `PathReciproc` and
   `PathSqrt`) and the finder (`Terrain/Pathfind.rs` plus `Pathfind/Directional.rs`) are all in, and
   the world installs them before its listener opens (`worldserver/src/main.rs`, port of
   `ASURA/crates/asura-worldserver/src/main.rs:366-391`). Kryta deliberately omits ASURA's archive
   fallback: it loads the shipped `PathGraphs` cache, binds whichever maps it contains, and a map with
   no graph answers `NoGraph` and nothing on the wire (`docs/decisions/0014-headless-navigation-assets.md`).
   The importer remains an offline management capability. It was proved against this machine's archive:
   map 148 - entry
   `0x0001B97D`, whose path chunk is `0x20000008` (`0xEEFE704C` signature, version 12) - walks from its
   own spawn point `(9541.87, 4796.46)` to `(9600, 4850)` with `Found`.
   The three move handlers broadcast their answer to the other players in the copy before the walker
   itself (`agent.rs:849-855`, `1041-1042`, `1103-1104`, `1171-1174`), which is `Peers::broadcast` here.

2. **The items block goes without `0x015E`** (the low-detail item), and so does ASURA's: the message is
   observed in a capture and emitted by neither implementation, so it is not this tree's to invent.
3. **The instance table carries the whole catalogue.** ASURA's batch shape goes on the wire and its
   content is every record the pack gives a value to: **137 pairs** in ten `0x01A0` batches - 17 of them
   hosted maps and 120 that are destinations only (`ASURA/crates/asura-world/src/instance_table.rs:63-71`
   keeps a record its own catalogue reader skips). A record with no type is a row like any other and
   `MapEntry::is_hostable` is the difference: a travel or a crossing that names one is refused, exactly
   as `catalogue::entry` answers `None` for it (`ASURA/crates/asura-world/src/catalogue.rs:54-63`).
4. **The travel is ported with its cycle and without the doorway.** The eight frames, the three
   triggers (`/tp`, the world map's `0x80B1`, the touch's `0x80C1`), the `is_explorable` switch and the
   hand-off row are ASURA's. Two parts are not, each named with its reason:
   - **The seat reservation** (`travel.rs:419-431`, `placement::Placement::reserve`) is ASURA's own
     deployment accounting - a world registry, leases and a heartbeat, which is the multi-process
     arrangement this tree's structure does not have (rule 0's scope is the protocol; the arrangement
     is the reference's). The capacity it enforces has a counterpart here: a district holds
     `World.DistrictAllotment` players, a full outpost opens the next district and a named full district
     is refused (`game/Districts/mod.rs`, `enter`, R5/R22/R58).
   - **The entry's facing is a row**: a spawn point now states the direction a character entering there faces (`map_spawn.angle`, ASURA `agent.rs:460-476`, `Standing::at_spawn`), so the create message carries the point's own vector instead of the neutral +y, and map 148's two points are the pack's own (`Outposts/148/spawns.json`), with their angles.
- **The doorway crossing** is ported (round 18): `game/Maps/Portals.rs` is `portals.rs` whole - the
     wall test, the point box, `Crossed`, and the three lookups - with the rows in place of the pack's
     `portals.json` (`map_arch` for arches, `map_arrival` for arrivals, which gains the facing as
     `angle`), and `cross_portal` (`game/Handlers/TravelHandler.rs`) is `travel.rs:319-388`: the step between two reports,
     the map's own walls, an arch that names no destination doing nothing, the receiving map's arrival
     as the landing, and the same `begin_transfer` every trigger ends in. The landing travels on the
     hand-off row (`travel.landing_x/y/plane/from`) and the arriving connection stands on it and faces
     along the arrival's own angle. On this machine: `Doorways: 1 arch(es) on 1 map(s)`.

**Two client-visible values this reading corrected.**

- `0x019A`'s explorable byte is the map's own kind: **0 for a town or outpost and 1 for an
  explorable** (`ASURA/crates/asura-world/src/map.rs:76-86`, `load_info_explorable`). Round 17 made
  it 1 for every load from an older ASURA build; that made Ascalon City load as an explorable. It is
  now 0 for map type 0 and 1 for every other hosted type, so the client keeps party changes in an
  outpost and closes them in an explorable.
- `0x01A6`'s `is_explorable` remains `World.TravelExplorable` (`0`, `1`, `auto`), defaulting to
  ASURA's `0`: at 1 an explorable's load waited 5138 ms against 849 ms at 0 (`travel.rs:146-176`).
  The destination's own kind is still what `0x019A` reports on arrival.




## 7a. The placement rule: ported whole, and the one datum it still wants

`game/Maps/Placement.rs` is now `ASURA/crates/asura-rules/src/lib.rs` (283 lines) whole: `Requested`,
`Character`, `Substituted` with its three reasons, `name_is_acceptable`, and `place` with all three of
its rules in their order - a creation visit is its own destination; the creation map named as a place
falls to the character's row, then its last outpost, then the caller's fallback; and **the outpost
rule** for every destination that is not a main town (`lib.rs:119-134`).

```text
3. The outpost rule, for every destination that is not a main town.
     destination_type is None                  -> the client's own map
     destination_type == main_town             -> the client's own map
     character.last_outpost == creation_map    -> the fallback outpost, NothingToPlace
     character.last_outpost == request.map_id  -> the client's own map
     otherwise                                 -> character.last_outpost, NotAnOutpost
```

**What is not settled is the datum, not the rule.** `destination_type` is the *catalogue's* type for
the map being asked for - the world's `map` row, in this tree - and the login program holds no world
catalogue: its configuration opens the login and character databases only
(`authserver/authserver.json`), and the reference's own auth server reads no world database either.
So the caller passes `None`, which is rule 3's own first branch ("the client's own map"), and the
program says so in its log. Nothing is invented to fill it.

Three ways to give it one, and the owner picks - this is a decision, not a piece of work:

1. **The login program reads the world database read-only** for a map's kind. Simple, and it makes the
   login depend on a third database the reference's login does not open.
2. **The character row carries the kind** of the map it stands in (a column, or the travel's own
   record), so the login answers from what it already reads.
3. **The rule is left out of the login** and applied where a catalogue exists - the world - which
   moves the substitution to the other side of the connection.

While the answer is `None`, a client that asks the login for an explorable is answered with that
explorable, and the world's load refuses a map it cannot host (`is_hostable`); the
substitution rule 3 describes is proved by `placement`'s own tests but is not in force.
## 7b. The walking subject, surveyed the way Rule 0a requires

The owner named walking. Under Rule 0a the survey comes before any estimate, so the four ASURA files
were opened and read for their shape, not their size:

| file | lines | what it is |
|---|---|---|
| `graph_dump.rs` | 473 | **the portable form of a path graph**: `DumpedGraph`, `encode`, `decode`, and its own little-endian container (`ASURAPG1`, version 1). ASURA's world loads this at start-up with `--path-graphs` so a server needs a few megabytes instead of a 3.6 GB archive. It carries **exactly what the reference's A\* consumes**: every plane, its trapezoids with neighbour and portal ids, its x/y split nodes and sink nodes, its portals with the paired portal on the other plane. It is deliberately our own format and not a `.pmap` reader, because a `.pmap` drops the portals and the node tree the finder needs. |
| `map_data.rs` | 1,196 | `PathStaticData` and the types the dump carries (`Plane`, `Trapezoid`, `XNode`, `YNode`, `SinkNode`, `Portal`, `NodeRef`, `Vec2f`), and the importer that builds them from one `Gw.dat` blob (`GmMapDataImport.c`, 590 lines, ported function for function, with three reference oddities kept on purpose). **Needed to *produce* a dump; not needed to walk from one.** |
| `pathfind.rs` | 1,794 | the finder over `PathStaticData`: `PathFindTrapezoid`, A\* over the trapezoid graph, the table-driven costs - `GmPaths.c`'s other half. |
| `pathfind/directional.rs` | 320 | the directional queries over the same graph. |

**What this corrects.** A previous session read `gwdat.rs` and built the archive reader in
`game/Pack/`, then reported that reading `Gw.dat` was the road to walking. It is not: ASURA's own
server walks from the dump, and the archive is the *dumper's* tool. The archive work stands on its own
(it proves a map's file id is real and lets a dump be produced here), but it is not on the walking
road.

**The order the subject therefore takes, largest-and-first:**

1. **(done)** **The types** `map_data.rs` defines - `Vec2f` (with `sub`/`cross`), `NodeRef`, `XNode`,
   `YNode`, `SinkNode`, `Trapezoid`, `Portal`, `Plane`, `PathStaticData` (with `plane_bounds`),
   `Bounds` (with `contains`), and the constants `PATH_DATA_SIGNATURE`, `PATH_DATA_VERSION`,
   `STAGE_SHIFT`, `STAGE_MAP`, the three node index bases and the three `NODE_TYPE_*` bytes - in
   `game/src/Terrain/MapData.rs`, same names, same fields, same order, proved by
   `terrain::map_data`'s three tests. The importer that fills them is step 4, below.
2. **(done)** **`graph_dump.rs`**, whole: the `ASURAPG1` container, `DumpedGraph`, `encode`, `decode`,
   `Cursor`, the three index/tag helpers, and its tests - in `game/src/Terrain/GraphDump.rs`, three
   tests green (a round trip of every field, a foreign or truncated or future-version file refused, an
   empty dump).
3. **(done)** **`pathfind.rs`** and **`pathfind/directional.rs`**, whole: the finder, with the
   reference's own quirks kept - `PathReciproc`/`PathSqrt` through the ported tables,
   `PathFindTrapezoid` by the same walk, no FPU substitutions, no tidying.
   Carried so far, in `game/src/Terrain/Pathfind.rs`: ASURA lines **33-454** - the vector helpers
   (`clampf`, `is_zero`, `equal`, `add`, `sub`, `dot`, `cross`, `div`, `vec2f_dist`, `vec2f_unit`),
   `Pos::v2`, `FindPoint`, `FindNode`, `HeapNode`, `BuildStep`, `heap_push`, `heap_pop` (the
   reference's own sift-down), `reverse_path_find_node`, `is_point_in_trap`, `closest_point_on_segment`
   (offset-not-point quirk kept), `dist_to_trap`, `find_point_on_next_trap`,
   `find_intersection_point`, `pick_next_point`, `precise_cross`; then lines **456-557** - `PathContext`
   with its `index`, `nodes`, `prioq` and `steps`, `new`, `trap`, `plane_of`, `neighbour`, `portal`,
   `portal_pair_traps`, `find_trapezoid`, `plane_at`; and from `map_data.rs` the two lookups the context
   is built on, `PathStaticData::trapezoid_at` (the `PathFindTrapezoid` split walk, `GmPaths.c:11`) and
   `trapezoids()`. Tests green: the heap's cost order, `precise_cross`'s near-parallel rule, the split
   walk down a plane's nodes to its leaf, and the pre-existing shim's. Trapezoid predicates are exercised
   by real graphs, not by invented ones; then lines **559-777** - the search's single expansion:
   `add_node`, `visit_trap` (the 0.01 estimate nudge kept), `visit_above`, `visit_bellow`, `visit_portal`
   (the pair's mirror asserted, the `0x4` flag refused, the corner choice by `left`, and the entry point
   from `pick_next_point`); then lines **921-1286** - `Bound`, `BuildHelper` and **its whole `impl`**:
   `run` (the corridor loop, the down-edge swap kept, MAX_LOOP asserted), `add_waypoint_and_reduce`
   (including the parallel-step case the reference leaves undefined, answered the way the one measured
   walk is answered), `add_waypoint`, `process_next`, `add_last`, `portal_adjacent`; and lines
   **779-918** - `PathFinding` (the A* loop: the same-trapezoid shortcut, the 10,000 cost ceiling, the
   four neighbour visits, the two portal visits, the closest-trapezoid fallback that still builds a
   path and still reports failure) and `PathCreateWaypoints`, which hands the chain to the corridor
   walk. **The finder's core is ported and proved**: six tests green, including two real walks over a
   two-trapezoid graph (one up into the neighbouring trapezoid, ending at the destination; and two
   points in one trapezoid being a single waypoint), and the split walk being the BSP's answer rather
   than a containment test - a point past a trapezoid's edge still lands in a leaf and still walks,
   exactly as the reference does; and lines **1484-1700** - `PathGraphs` and `PathGraphsInner` with
   `from_dump`, `bind`, `empty`, `imported`, `maps`, `walk`, `walk_any_plane` and `with_finder`. A
   test loads a graph from a real `ASURAPG1` dump, binds it to one of two maps, and walks: the bound
   map answers `Found`, the unbound one `NoGraph`, and `walk_any_plane` finds a destination on an
   unspecified plane; and **all 320 lines of `pathfind/directional.rs`** - `DirectionalSettings` (its
   two numbers are ASURA's `directional_movement.json`: `edge_tolerance` 0.01, `distance_slack` 0.5),
   the edge/coverage/neighbour helpers, and `walk` itself, with ASURA's own **PROVISIONAL / NOT
   ACCEPTED** caveat carried in the module header rather than tidied away - plus
   `PathGraphs::walk_direction` (1639) wired onto it. A test walks an upward offset across two
   stacked trapezoids and checks the map with no graph still answers `NoGraph`; and lines **1746-1794**
   - `find_path_blob`, the archive's bit-31 replacement rule (the configured id, then the plain one,
   then the flagged one), reporting a substitution with the working implementation's own line.
   **Left**: nothing. `PathGraphs::load`/`import` (1507-1592) and `import_graph` (1703-1744) wait
   on `PathStaticData::import` in `map_data.rs` (step 4), and are ported with it; the world's
   start-up and the session's two walk answers use them.
4. **The importer** in `map_data.rs` - the last piece, and its own session's work. Surveyed function by
   function (ASURA's line numbers), so the port starts from this map rather than from the file:
   - **(done)** `Reader` and its `new`/`len`/`take`/`tagged` (336-383), the `u16_at`/`u32_at`/`f32_at`
     reads (385-396), `riff_type` (398), the RIFF container (`riff_type`, `RIFF_SIGNATURE`,
     `RIFF_TYPES` = 9, `MapChunk`, `Chunks::parse`/`get`/`iter`/`len`/`is_empty`, 408-455) and
     `node_ref` (457) - the dat node id to `NodeRef`, past the array's end answering `None`. All in
     `game/src/Terrain/MapData.rs`, with the `ImportError` they answer with and its `Display`.
   - **(done)** `trap_ref` (472) and, of the plane readers, `read_counts` (578), `skip_tag11` (606 -
     the `h000C * 8` of a `sub_length` section, the reference's own arithmetic kept with its
     measurement in the comment), `read_vectors` (618), `read_trapezoids` (639 - the `trap_id` counter
     and the four neighbour `trap_ref`s), `read_root_node` (681), `read_xnodes` (705), and the
     `vector_at` they all read through (911), with `Trapezoid::SIZE` = 44.
   - **(done)** `read_ynodes` (732), `read_sink_nodes` (753), `read_portal_traps` (786), `read_portals`
     (816 - the `(plane << 16) | shared_id` pairing), plus `ChunkId`/`PATH_CHUNK_ID`,
     `RIFF_MAP1`/`RIFF_MAP2`, `PathStaticData::import` (490), `read` (506 - the signature/version check
     and `PathImport_Step2`'s length read from byte 1 of five), `read_planes` (535) and `read_plane`
     (551). **The importer is ported.** Four tests: ASURA's container refusal (946), its record-past-the
     -blob case (971), its node-id ranges (994), and against this machine's real archive - map 148's
     blob imports, the graph has planes and trapezoids, and the map's own spawn point lands in a
     trapezoid, which is the check ASURA uses to know its import worked.
   - **Left**: nothing of the importer. **(done)** The three callers too: `import_graph`
     (pathfind.rs 1703-1744, with the working implementation's own log lines for a substituted id, an
     unimportable blob and an absent one), `PathGraphs::load` (1507) and `PathGraphs::import` (1562).
     A test loads map 148 from this machine's real `Gw.dat` through `PathGraphs::load`, binds it, and
     **the map walks from its own spawn point** - while map 146, unbound, still answers `NoGraph`.
     **The walking subject is ported end to end and wired in**: before its listener opens, the world
     program reads its required extracted `PathGraphs` cache (`worldserver/src/main.rs`). It never
     opens `Gw.dat`; the manager performs a selected-map export offline. `World` holds the resulting
     graphs and `WorldSession` answers `0x803E` and `0x803D` from them.
   - `PathStaticData::import` (490) and `read` (506): the `PATH_DATA_SIGNATURE`/`PATH_DATA_VERSION`
     check, then `read_planes` (535) -> `read_plane` (551) -> per plane `read_counts` (578),
     `skip_tag11` (606), `read_vectors` (618), `read_trapezoids` (639), `read_root_node` (681),
     `read_xnodes` (705), `read_ynodes` (732), `read_sink_nodes` (753), `read_portal_traps` (786),
     `read_portals` (816), and `vector_at` (911).
   - The three reference oddities the file's own header names and the port must keep: `skip_tag11`
     advances `h000C * 8` bytes of a section measured as `sub_length` (the two differ in every plane of
     every map measured, 283 planes), `node_ref`/`trap_ref` answer `None` past their arrays instead of
     failing the import, and `read_plane` takes a length from byte **1** of a five-byte header.
   - ASURA's own tests to port with it: a blob that is not a container refused by its first five bytes
     (946), a record past the blob refused (971), a node id naming its array's range (994), a
     trapezoid naming itself as its neighbour answering `None` (1025), the spawn points of the
     reference's configured maps landing inside the graphs in the archive (1045), and a plane walking
     to the position's trapezoid and stopping at a dead end (1146).
   `import_graph` (pathfind.rs 1703-1744) and `PathGraphs::load` / `PathGraphs::import`
   (1507-1592) remain archive-import tools; `worldserver` instead reads the extracted dump through
   `PathGraphs::from_dump`.

Nothing in 1-4 is restructured or improved. The types keep ASURA's names and field order; the search
keeps its traversal order, its tie-breaking and its quirks; the dump keeps its byte layout.

## 7c. The party subject

**Opened:** `game/Parties/mod.rs` and `game/Parties/Rules.rs`, with the handlers that answer them -
`Handlers/ChatHandler.rs` for `/resign`, `Handlers/TravelHandler.rs` for the world-map travel, its
countdown, its cancel and the defeated leader's return, and `Server/Packets/PartyPackets.rs` for every
frame. **Verdict: ported for the outpost and travel flows**, with the pieces under the table named as
not here.

| area | what | this tree | verdict |
|---|---|---|---|
| sizes | the pack's `party_sizes.json` | `World::load_party_sizes`, `World::max_party_size`; a map the file does not list takes `PartySizeCap` (R15, default 8) | ported |
| load tail | the party state a load ends with | `parties::load_tail`, `splice_load_tail`, inserted before the finish | ported |
| table | a party, an absent member, one table per running copy | `Party` (id, leader, members, absent, moving, countdown, resigned, dead, defeated), `Absence`, `Table`, and `Parties` per `(map, district)` | ported |
| frames | the member stream, the size-and-leader pair, the load section, the in-map notices | `member_stream`, `size_and_leader`, `load_section`, `splice_load_section`, `in_map_notices`; delivery through `Peers::send_to` | ported |
| requests | invite, withdraw, decline, accept with its merge, leave, kick | `handle_invite`, `handle_cancel`, `handle_refuse`, `handle_accept`, `handle_leave`, `handle_kick`, each answering a plan | ported |
| departure | what the member, its party and the bystanders are told | `departure`, `left_instance` | ported |
| resign | `/resign`, the delayed death, the party's defeat, the leader's return | `ChatHandler::answer_resign` marks the member and dies after `World.ResignSecs`; `0x01D9` is sent when every present member resigned; `TravelHandler::answer_return_to_outpost` answers `0x80A7` from the leader's `last_outpost` | ported |
| travel as one | a doorway crossing or a leader's world-map travel moves the party | `World::queue_travel` asks each present member's session to run the same redirect; `World::hold_party`/`take_held_party` and `Table::adopt`/`arrive_member` carry the roster; the destination district is chosen on arrival | ported |
| leader countdown | the lock, the timer, the two confirmation questions, the cancel | `TravelHandler::answer_party_travel` sends `0x01E7 {4}`/`{5}` when it must ask, holds the roster, sends `0x01DA {2, 1}` and `0x01BC` with the map's `name_id`, and the timer moves the party after `World.CountdownSecs`; `0x80A3` cancels it and sends `0x01DA {0, 0}` | ported |
| leave map | a logout, a drop or a travel | `left_instance`: an explorable with company keeps the member as the grey shadow (`0x00B1 {0}`, `0x01CE`); elsewhere the member is removed and its invitations are closed | ported |

What is not here: the database party ledger (a party crosses a map in this world's memory, through
`World::hold_party`, so a restart or a second world has no roster); the timed expiry of a held place
(`rules::holds_on_disconnect` is tested, but `left_instance` keeps the shadow until the copy ends); the
truncation of a party that arrives at a smaller cap (`rules::travelling_count` is tested, but the
travel handler does not yet drop the tail into parties of one); `present_characters`; heroes and
henchmen; party search and the difficulty toggle. These are named so the next round starts from them,
not from a claim that the subject is whole.

Every party batch also reaches the other players through `Peers::introduction`, whose size-and-leader
pair reads the party table and whose burst is the whole introduction (professions, level, flags word or
title, effects, the items it wears, visual equipment and cape), sent after the player's own block. The
guild block arrived with 7e.

## 7e. The guild and experience subjects, from ASURA `071735e`

**Opened:** the guild subject is five ASURA files - `asura-world/src/guild.rs` (819 lines),
`asura-protocol/src/game/guild.rs` (665), `asura-rules/src/guild.rs` (277), `asura-db/src/guilds.rs`
(343) and `migrations/0029_guilds.sql`, with `config/Server/guilds.json` - and the experience subject is
`asura-rules/src/experience.rs` (124), `asura-world/src/experience.rs` (89), `player.rs:341-353` and
`:460-569`, `chat.rs:288-302` and `:612-648`, `migrations/0036_character_experience.sql`, with
`config/World/Vanilla/experience.json`. **Carried:** all of them. Neither subject exists at `ca9f532`, so
the counts in 6 do not include them. What each message is, with its capture, is ASURA's
`docs/41-guild-system-research.md` and `docs/54-experience.md`.

| ASURA | this tree | verdict |
|---|---|---|
| `asura-protocol/src/game/guild.rs` - every builder (`0x0118`, `0x0119`, `0x011C`, `0x011F`, `0x0121`, `0x0127`, `0x0128`, `0x012F`, `0x0131`, `0x0132`, `0x0133`, `0x00CC`), the registration window (`0x00C4`, `0x0084`, `0x00CA`, `0x00C3 {8, 0}`), the four readers, the date encoding, every test | `game/Server/Packets/GuildPackets.rs` | ported |
| `asura-rules/src/guild.rs` | `game/Guilds/mod.rs`, the rules half | ported |
| `asura-world/src/guild.rs:35-200` - `Membership`, `Badge`, `LocalIds`, `badge_frames`, `load_frames`, `splice_into_player_block` | `game/Guilds/mod.rs`; the local ids are one table per `(map, district)` in `Peers` | ported |
| `asura-world/src/guild.rs:202-710` - `/guild create`, `0x8036`, `0x80B5`, `0x80BB`, `0x80BE`, each batch in ASURA's order | `game/Handlers/GuildHandler.rs`, dispatched as `dispatch.rs:116-129` dispatches them | ported |
| `instance.rs:1047-1051` - a guilded peer's badge in its introduction; `connection.rs:405-449` - the guild read at load; `player.rs:740-756` - the guild block before `PARTY_CREATE` | `Peers::introduction`, `WorldSession::load_guild`, `splice_guild_block` | ported |
| `asura-db/src/guilds.rs`, `0029_guilds.sql` | `game/Guilds/Repository.rs`, `data/sql/base/db_characters/0009-guilds.sql` | ported, three differences of arrangement: ids are the `uuid` text every character table here uses, numbers are `BIGINT`, and the two character columns carry no foreign key (the file sorts before `characters.sql`), so `DatabaseCharacters::remove` deletes them as ASURA's cascade does. `create` is one statement instead of a two-statement transaction |
| `asura-config/src/lib.rs:916-966`, `Guilds` | `guilds::Settings`, `config/Server/guilds.json` (`GuildsFile`) | ported; a missing file stops the program (rule 7) where ASURA refuses each request |
| `asura-rules/src/experience.rs`, `asura-world/src/experience.rs` | `game/Characters/Experience.rs`, `config/World/Vanilla/experience.json` | ported |
| `player.rs:341-353` - the progress record carries the row's experience and skill points | `Agents/Player.rs`, `progress_record` | ported |
| `player.rs:462-569` - `set_experience`: the row first, then `0x00EE {0, delta}` for a rise of one level or less, the whole `0x00E9` otherwise, nothing else | `Handlers/CharacterHandler.rs`, `answer_experience`; `0x00EE` is `character_packets::agent_kill_reward` | ported |
| `chat.rs` - `/guild`, `/.exp <total>`, `/xp`, `/xp <total>` | `config/Server/commands.json` rows `guild`, `.exp`, `xp` | **ported thin**: one name is one row here (`Chat/Commands.rs`), so `/xp` only reads and `/xp <total>` answers "Unknown command"; `/.exp <total>` sets |
| `0036_character_experience.sql` | `data/sql/base/db_characters/0008-character-progress.sql` (`experience` was already a column) | **ported thin**: ASURA's backfill of existing rows is an `UPDATE`, which a schema file may not run here (rule 7), so a character created before this file keeps experience 0 until its next `/.exp` |

What ASURA itself does not do is not done here either, and is listed so it is not mistaken for a gap of
the port: gold for creation and invitations (the prices are placeholders, read and logged), the creation
batch's server chat line, the invitee's `0x0123` and the accept, kicks, promotions, guild chat, guild halls,
capes, the roster and history a load carries beyond the founder's row, telling other members of an
announcement, and experience from kills or quests.

## 7d. The cartography subject

**Opened:** Preforged's exploration implementation (`docs/exploration.txt`,
`src/runtime/gw_exploration.h`, `gw_exploration_rects_generated.h`,
`gw_world_map_dimensions_generated.h` and `tests/exploration.cpp`) for the feature, and Rurik's
`toolkit/authsrv/fogrle.py` with ASURA's `cartography.rs` for the modern wire. **Carried:** the
content, the grid, the run-length stream, the load pair, the reveal mark, the per-character save and
the walk sample. **Verdict: ported for the maps the content can place**, thin where a map has no world
bounds yet.

| piece | what | this tree |
|---|---|---|
| content | nine continent grids and each map's rect in terrain cells; optional world bounds from the map's Map Parameters chunk | `config/World/Vanilla/cartography.json` (492 map rects, 31 with bounds), `Cartography::load` |
| grid | one bit per 32-unit block, row-major, LSB-first | `Grid` in `game/Maps/Cartography.rs` |
| stream | 16-row bands, `u16` byte length, empty all-fog bands, fog-first, the first run actual and every later one shorter, `0xFF` additive, every non-empty band written to its full width | `Grid::encode` / `Grid::decode`; retail's own stored init (`maps-live-20261123-022154.jsonl` seq 654-655: 55 bytes, bands 0/27/12/0..., 107 cells) is reproduced **byte for byte** |
| load | `0x008B` dims and byte count, then `0x008A` slices of at most 64 dwords | the **empty** grid's frames right after `0x019A`; the character's saved blocks replay as `0x008C` marks after the load, filtered to the loaded map's rect. A stored non-empty init corrupted the agent block (health/mana) and was refused with 007 on this client |
| reveal | `0x008C {x, y, half-span}` - 11 bytes on the wire, byte span; not the 2022 10-byte mark and not the client sniffer's 16-byte struct (four-byte header and fields) | `instance_packets::cartography_mark`; the session sends one mark per changed 3 x 3 window, clamped to the map's own rect |
| sample | authoritative position every 200 ms; one cell per 96 game units; y inverted | `WorldSession::sample_exploration`, `MapArea::cell` |
| save | one row per character and continent, raw words | `data/sql/base/db_characters/0012-character-exploration.sql`, `DatabaseCharacters::{exploration, record_exploration}` |

The direct evidence for the load is the retail capture of 2026-11-23
(`D:\ASURA-main\recordings\client\feature\maps\maps-live-20261123-022154.jsonl`): `0x008B` seq 654
declares `64 x 128` and 55 bytes, `0x008A` seq 655 carries that non-empty stream, and the `0x008C`
marks land on cells it decodes. The stream's bands are `0, 27, 12, 0, 0, 0, 0, 0`; under fog-first
+1 it is 107 cells, every retail mark checked is set, and `Grid::encode` reproduces the 55 bytes
exactly. The later loads in the same capture grow the grid 107 -> 111 -> 115 with no shrink, so the
init is the character's stored state, not a constant. **This tree does not send that stored init on
this client yet:** a non-empty init corrupted the agent block (the health/mana bars) and the sparse
grid was refused with 007, while the same saved blocks arrived as marks and the empty init loaded.
The accepted path here is therefore the empty init plus filtered replay marks; the byte-for-byte
stored-init codec stays tested for the day the client-side condition is understood.

A map whose `cartography.json` row has no `bounds` still gets the grid (the client asserts without the
pair), but the session does not invent a position for it: nothing is revealed until its Map Parameters
rect is read from an archive that still holds the map. The start-up line states how many maps are
placed; the load's log names the grid size and explored cells.

**Not carried:** the client's per-map explorable mask (the server cannot mark a block the loaded map
refuses, and what produces that mask is not read); the Cartographer title's percent sync; merged
horizontal spans (`span` is always 1); and the exploration of a continent whose grid dimensions the
content has not measured. The last is why an unlisted map falls back to Tyria's 64 x 128 grid instead
of guessing another continent's.

## 7g. The NPC subject: stages N1 and N2 of `docs/asura-port-plan.md`

**Opened (2026-11-22, the local ASURA repository as it stands on disk):** `crates/asura-world/src/npc/`,
nine files - `table.rs` 1,255 lines, `behavior.rs` 840, `content.rs` 832, `fight.rs` 663, `file.rs` 486,
`burst.rs` 439, `lua.rs` 481, `mod.rs` 169, `frames.rs` 135 - and the NPC builders of
`crates/asura-protocol/src/game/mod.rs`. **Carried:** `burst.rs` 1-439, `frames.rs` 1-135,
`content.rs` 1-832, `file.rs` 1-486 (whole, each with its tests), `mod.rs` 90-135 (the start-up read
and the ground check) and `pathfind.rs` 1682-1686 (`on_ground`). **Carried with N4 (2026-11-22):**
`behavior.rs` 1-840 except the Script leaf and its Lua state, `table.rs` 26-48 (`BehaviorSource`,
`PackBehaviors`), 50-197 (the burrow cycle), 278-310 (`tree_for`, `check_trees`), 639-718 (status,
created set, hidden bodies) and 720-879 (the body tick, walks and per-viewer deliveries), each with
its tests. **Carried with the Lua source (2026-11-23):** `lua.rs` 1-481 whole, with its six tests
(`game/Scripting/NpcBehaviors.rs`); the Script leaf of `behavior.rs` (72-79, 93-111, 168-169, 196-204,
244-250, 300, 358-398); `table.rs` 32-37 (`BehaviorSource::script`) and 347-351 (the script-owned
burrow at seed); `mod.rs` 55-88 (the source chosen once, a pack refused on any tree problem, in
`World::load_npcs`); and ASURA's `config/World/Vanilla/npcs/behaviors.lua`, copied unchanged.
**Left:** `fight.rs` (so `ctx.in_combat` is always false, and `table.rs:779-787`'s fight gate has
nothing to gate), `table.rs` 531-637 (`/npc`), and `mod.rs` 1-54 (`NpcWorld`, whose job
`World` and `NpcDistricts` already do).

| ASURA | this tree | verdict |
|---|---|---|
| `game/mod.rs` 1426-1555, 1581-1679, 2274-2281: `CreateAgent` (+ `h000b`, `h001e`, `h0023`, `h0027`, `LIVING`), `CHAR_CLASS_NPC_BASE`, `NpcProperties`, `npc_update_properties`, `npc_update_model`, `agent_update_npc_name`, `agent_update_scale`, `npc_update_weapons`, `agent_update_speed_base`; the test `the_npc_messages_are_the_captured_bytes` | `game/Server/Packets/AgentPackets.rs` | ported, with the capture-byte test; the two player creates (`Agents/Player.rs`, `Districts/Peers.rs`) finish with `..CreateAgent::LIVING` and send the same 99 bytes as before |
| `npc/burst.rs` (the measured entry order, `held_item_id`, `entry`, `entry_with_status`, `BURROW_TRANSITION`, `burrow_recreate`) and `npc/frames.rs` (`Msg` to frame) | `game/Server/Packets/NpcPackets.rs` - the reference's `NPCPackets` family | ported whole with their 8 tests (seq 246-252, 265, 294, 573-580, the armed body, the foe's 42/34, the pace pair) |
| `npc/content.rs` (`FourCc`, `NpcDefinition`, `NpcCombat`, `NpcAttack`, `Aggression`, `Property`, `NpcPlacement`, `Route`, `MapNpcs`, `SightRules`, `NpcContent::assemble`, `check_ground`) | `game/Agents/NpcContent.rs` | **ported thin:** everything but `content.rs:448-453`, the `damage_type` check against the skill model's damage kinds, which waits for the skills subject. The nested `if let`s are let-chains (clippy, edition 2024); same logic |
| `npc/file.rs` (the pack's `npcs/definitions.json` and each map folder's `npcs.json`, every refusal) | `game/Maps/NpcPack.rs`, re-exported at `game/Agents/NpcFile.rs` | ported, this tree's arrangement: rows are read from `serde_json::Value` with ASURA's unknown-key refusal kept (this tree's four commentary names are walked past); **`npcs/sight.json` is not read** - the compass range is `World.CompassRange` (R34, the same 10,000), handed to `load`. The reader lives beside `Maps/Pack.rs` because it reads the disk at start-up, which `worldserver/tests/content_is_held.rs` forbids inside a connection-serving folder |
| `npc/mod.rs` 90-135 (`current`, `check_ground`), `main.rs` 403-412 and 479-485 | `World::load_npcs` (after the settings), `World::check_npc_ground` (after `set_graphs`), `worldserver/src/main.rs` | ported: a bad row refuses the start naming the row; a moving body or route point off the surface refuses it; a standing one or a graphless map is a note |
| `pathfind.rs` 1682-1686 `on_ground` | `PathGraphs::on_ground`, `game/Terrain/Pathfind.rs` | ported |
| `npc/table.rs` (per-instance table, ids before players, sight churn, dormancy, the burrow cycle, the tick) and the NPC half of `instance.rs` (`with_npcs` 178-225, `join`'s agent id 307-316, `npc_arrival` 450-460, `leave` 424) | `game/Districts/Npcs.rs` (`NpcTable`, `NpcDistricts`), `World::allocate_agent`/`release_agent`/`npc_arrival`/`npc_tick`/`npc_sweep`, the load's agent array and the session tick in `WorldSession.rs` | **ported thin (N3-N4, 2026-11-22):** a copy's bodies are seeded from agent id 2 the first time it hands out an agent; a player's agent is the copy's lowest free id, **separate from its player id** (retail: `pre-searing-load-3.jsonl` seq 221, player 0x35 is agent 40); the load's agent array is other players and NPCs in range sorted by agent id (`player.rs:784-818`); the burrow cycle, the status word, the created set, dormant bodies and the walk half of `tick` are carried. The clock is adapted to this tree's per-connection loops: a copy stores its last body tick and only the first connection to reach `SimulationTickMs` advances the bodies, so two players do not run the town twice as fast. Not here: the Script leaf, the fight, `/npc`, quest markers. Every place that named the player id where an agent was meant now names the agent: `0x019A`, the load's `0x0020`, the travel's `0x00F1`/`0x0028`, `0x006E`, the guild badge and property 30, the registrar window. Client proof pending |
| `npc/behavior.rs`, `PackBehaviors` | `game/Behaviour/mod.rs` (`Tree`, `Spec`, `Ctx`, `Patrol`, `Script`, `PackBehaviors`) | **ported (2026-11-23):** `Sequence`, `Fallback`, `Wait`, `Stand`, `Patrol`, `Burrow`, the Script leaf and its per-agent Lua state (`behavior.rs:72-79`, 93-111, 168-169, 196-204, 244-250, 300, 358-398), the clock, the failure reasons and the 7 tests |
| `npc/lua.rs` | `game/Scripting/NpcBehaviors.rs` | **ported (2026-11-23):** whole, mlua 0.10 / Lua 5.4 vendored, sandboxed to `table`, `string`, `math`, with its 6 tests. `World::load_npcs` chooses the source once (`mod.rs:55-69`) and refuses a pack on any `check_trees` problem (`mod.rs:74-88`); `worm_lakeside` and `worm_queen` build and run |
| `npc/fight.rs` | `game/Combat/mod.rs` | ported (2026-11-23) with the skills subject, 7i |

**`behavior.rs` function by function (840 lines), so the next session starts from the map:** 33-42
`Status`; 44-80 `Spec` (six node kinds); 82-91 `BurrowRequest`; 93-111 `Script` (the Lua leaf, still
missing); 113-117 `Pathing`; 119-140 `Ctx`; 142-170 `Tree`, `Node`, `Kind`; 172-265 `Tree::build` and
`add` (route resolution, one `Burrow`, empty control nodes refused); 267-356 `tick`, `tick_node`,
`tick_control`, `halt_node`; 358-398 `tick_script` (missing with `Script`); 400-410 `stop_walk`;
412-432 `Patrol` and `Phase`; 434-584 `Patrol::new`, `pick_other`, `tick`, `advance`, `start_leg`,
`halt`; 586-840 the seven tests. The port keeps every line of 33-356 and 400-584; 93-111 and 358-398
are the only missing region.

**The action vocabulary, mapped to ASURA rather than invented.** An idle body is a placement with no
`behavior` (`Mind::Still`) or a Lua tree's `{ kind = "stand" }`; a patroller is `{ kind = "patrol" }`
over a route; a burrower is `{ kind = "burrow" }` or a script-owned cycle. Combat is **not** a tree
node: ASURA's `fight.rs` owns `State::Idle`/`Engaged`/`Returning`/`Dead` (`fight.rs:130-141`) and the
attacks on `crate::skill::engine::Engine`, and it pauses an ordinary tree while it drives
(`table.rs:778-788`). ASURA has **no flee state** and this tree has invented none; a Lua script can
decide to walk away once the Script leaf exists.

**The pack content is carried.** `config/World/Vanilla/npcs/definitions.json` holds ASURA's 114
definitions from preforged's gw-npcs catalogue (35 with a combat block), and eleven map folders carry
377 placements and 2 routes; the start-up line reports `114 NPC definition(s), 377 placement(s) and 2
route(s) on 11 map(s)`. The `speed` field is documented in ASURA as "cmonmeow's NPC value" (220) for
catalogue rows - an authored number, not a measurement - and the map 148 patrol is authored too, as
its own file says. Each file under `config/` is classified in `04-configuration.md`.

**Not carried, as the plan says:** `walk_speed` (`0x0027` + `0x002B` before a leg; it caused `007` on
client 39167) - the builder and the `Pace` message are ported, nothing emits them; `h003B` goes out as 0
where the live load varies.

**What walks now:** the two map 148 placements whose `behavior` is `patrol` walk their authored routes
while a client is inside `World.CompassRange`; a body with no behavior still stands. The nineteen
Lakeside worm placements name `worm_lakeside` and `worm_queen`, which are Lua script entries in the
pack's `npcs/behaviors.lua`: the Lua source builds them, the worms run ASURA's burrow cycle (up 5-10 s,
under 5-15 s, the measured 2.00 s transitions) and the Queen emerges once and stays up. Start-up now
refuses a pack with any `check_trees` problem, as ASURA does.

## 7h. The item catalogue and inventory rules, from the local ASURA tree

**Opened:** `ASURA/crates/asura-world/src/inventory_model.rs` (917 lines), `item_catalogue.rs` (636),
`item_modifier.rs` (536) and, for the second round, `item.rs` (1,298), read from `D:\ASURA-main` on
2026-11-22, with the pack's `items.json` (546 definitions) and `upgrades.json`. **Carried:** all three
rules files and both pack files, then `item.rs`'s five-step core, its four row-writing handlers and
its `/item` test command.
None of these exists at `ca9f532`, so the counts in 6 do not include them. The rule is
`docs/asura-port-plan.md`, row 6.

| ASURA | this tree | verdict |
|---|---|---|
| `inventory_model.rs:16-572` - `ItemKind`, `Hands`, `equip_slot`, `BagKind`, `BagInfo`, `Entry`, `Wearer`, `Op`, `Change`, `Refusal`, `InvModel` (`new`, the lookups, `first_free_carrying`, `apply`/`plan`, `do_move`, `do_equip`, `do_take`, the relocation helpers) | `game/Characters/Inventory.rs` | ported whole, each item citing its line; `game/Characters/ItemOps.rs` builds the model from the rows and applies one operation per client message |
| `inventory_model.rs:574-917` - the 22 tests | the same file | ported unchanged; 22 pass |
| `item_modifier.rs` - `ModifierWord` (both views, `skipped`, `decode`), `Modifier`, the tooltip lines, `ModifierEffect`, `fitted_upgrades`, `decode_all`, the 12 tests | `game/DataStores/ItemModifier.rs` | ported whole; 12 pass |
| `item_catalogue.rs:24-365` - the constants, `ItemDefinition`, `Upgrade`, `stack_limit`/`can_equip_in`/`accepts_quantity`, `Catalogue` (`from_json`, `with_upgrades_json`, the lookups, `by_wire`), `validate` | `game/DataStores/ItemCatalogue.rs` | ported; the reader is `serde_json::Value` with each object's keys stated and any other key refused, where ASURA derives `deny_unknown_fields` - same fields, same defaults, same refusals |
| `item_catalogue.rs:367-384` - `load` | the same file, called once by `World::load_item_catalogue` before the listener opens | ported; ASURA's `current()` (`:391-409`) reads lazily on first use and `path()` (`:386-389`) asks its spawn module - neither is carried, because a request path reads no file here (`worldserver/tests/content_is_held.rs`) and a missing `items.json` stops the program (rule 7) |
| `item_catalogue.rs:278-291` - `for_item(&t_item row)` | `Catalogue::for_item(definition_id, file_id, item_type, model_id)` | ported: the stored definition id is read first, and a row written before the column falls back to its wire fields. The column is `data/sql/base/db_characters/0010-item-definition-id.sql` (schema only; `''` for a row that names none) |
| `item_catalogue.rs:418-636` - the 10 tests | the same file | ported; `every_reference_seed_item_names_a_definition` and `a_row_with_an_id_resolves...` read `config/World/Vanilla/creation/inventory.json` where ASURA reads `asura_db::inventory::seed::ITEMS` (the same nine reference rows: every one resolves, with its profession). One test of this tree's own reader beside them; 11 pass |
| `config/World/Vanilla/items.json`, `upgrades.json` | the same paths, copied unchanged | ported as data, classified in `04-configuration.md`; `live: true` (497 of 546) is the field that marks a definition the client's item probe showed |

**The rules are wired now, and all six client item operations run through them.** The second round of the
`item.rs` re-diff carried the ground items and the two operations that need them:

| `item.rs` lines | what | this tree | verdict |
|---|---|---|---|
| 237-468 | the five-step order: `model_of`, `entry_of`, `row_changes`, `commit`, `commit_ops`, `reload`, `snap_back` | `game/Characters/ItemOps.rs`, with six tests | ported; `apply_item_changes` writes one operation's rows in one transaction (`database/src/Database/Pool.rs`, `transaction_reply`, the reference's `apply_changes`) |
| 483-578 | `drop_item` | `Handlers/ItemHandler.rs`, `answer_drop_item`, through `Op::Take` | ported: a whole stack answers `0x014D`, part of one answers `0x0139` + `0x0162`, both then the item agent (`0x0020`), the drop line and the ground record for the rest of the instance |
| 593-694 | `interact_item`, `pick_up` | `Handlers/ItemHandler.rs`, `answer_interact_item` and `pick_up`, through `Op::Merge` then `Op::Insert` | ported: the action hold and target (properties 8 and 39), the pickup line, `0x0159`, the stack's `0x0139` or the item's `0x013E`, `0x0021`, and the hold cleared |
| 704-822 | `move_item`, `equip_item`, `unequip_item` | `Handlers/ItemHandler.rs`, through `Op::Move`/`Op::Equip`/`Op::Unequip` | ported: `0x014B` per change, `0x006F` per worn slot, the bulk `0x006E` for this client on a drag, and a refusal answers where the item really is |
| 830-858 | `destroy_item` | `Handlers/ItemHandler.rs`, through `Op::Destroy` | ported; the `0x8069` dispatch arm was missing before |
| 867-1035 | `grant_definition` (`/item`), `clear_carrying`, `instantiate`, `equipment_frame`, `DROP_OFFSET` | `/item` in `Handlers/ItemHandler.rs` through `Op::Insert` and `Commands/Operations.rs` + `config/Server/commands.json`; `instantiate`, `item_of_row`, `equipment_frame` and `DROP_OFFSET` in `ItemOps.rs`/`ItemHandler.rs` | **ported thin**: `/item` is carried (`0x0161` + `0x015A` + `0x014B`); `clear_carrying` is not |
| 1041-1278 | the seven dispatch handlers | all seven arms: `0x8091`, `0x802C`, `0x8069`, `0x803F`, `0x8072`, `0x8030`, `0x804F` | ported |

**Ground items have a home.** `game/Districts/Ground.rs` is `instance.rs:117-151` and `:428-510`:
`GroundItem` (agent, item, the source stack, where it lies), the per-copy registry, `put`/`take`/`items`
and `close`, and `ground_frames`. `World` owns it (`put_ground_item`, `take_ground_item`,
`ground_arrival`, `allocate_ground_agent`, `release_ground_agent`), reuses the NPC table's lowest-free
agent id so an item cannot collide with a player or a body, and drops a copy's ground items when the
copy is disposed. A client entering the copy is told about them after the agent array
(`player.rs:855-862`). A partial drop's id comes from `item_id_seq`
(`data/sql/base/db_characters/0011-item-id-sequence.sql`), the sequence ASURA's own allocator uses;
`/item` draws from it too.

**One rule the ASURA server does not state, and this one does.** An item is never droppable in an
outpost: the client itself refuses to offer the drop outside an explorable, and destroying a bag is
allowed in outposts where dropping is not (`docs/game-rules.md` E51). ASURA's `drop_item` has no map
check because no client sends the message there. This tree refuses it on the server too - a silent
refusal, since no message acknowledges a drop (`E47`) - and `drop_allowed` is unit-tested for an
explorable, an outpost, the creation instance and no map kind.

**Proved against the databases, not only unit tests.** `a_travel_record_decides_the_map_and_the_district`
now drives a real connection through the rules: a drag answers `0x014B` and moves the row, an equip moves
the piece into its worn slot and answers `0x006F`, an unequip moves it back and clears the slot, a destroy
answers `0x014D` and the row is gone, `/item birthday_cupcake 2` gives two of a live definition
(`0x0161` then `0x014B`, the row written with its `definition_id`), a partial drop answers `0x0139`,
`0x0162` and its agent, the pickup merges it back and answers the stack's `0x0139`, then a whole-stack
drop answers `0x014D` and its agent and the pickup restores the row under the same id with `0x013E`.
The test's map is an explorable, because a drop is refused in an outpost; the refusal itself is the
`drop_allowed` unit test. The test is `#[ignore]` because it needs the login and character databases.

**The next round, in this order:**

1. **`/item clear`** (`item.rs:968-1004`): `clear_carrying`, which destroys everything in the carrying
   bags through the same rules and transaction.
2. **The load block's diff** (`item.rs:22-236`): `items_block`, `Purse`, `items_block_with_stream` and
   `inventory_block` against `game/Agents/Player.rs`'s items answer, which already draws the same stream
   from rows.
3. **The ground item's reservation** (`309`, `E48`) and gold drops (`E49`), neither of which ASURA
   carries: stops, not gaps.


## 7i. Skills, from the local ASURA_testfork tree, with Lua scripts

**Opened (2026-11-23, `D:\ASURA-main`, ASURA_testfork `071735e`):** `crates/asura-world/src/skill/` - `engine.rs`
2,637 lines, `tests.rs` 956, `catalog.rs` 751, `character.rs` 744, `model.rs` 531, `wire.rs` 386, `mod.rs` 291 -
with `asura-rules/src/skills.rs` 620, `asura-protocol/src/game/skill.rs` 538, `asura-db/src/skills.rs` 85 and
migration `0034`. **Carried:** `model.rs`, `engine.rs`, `tests.rs`, `catalog.rs`, `wire.rs`, `rules`, the protocol
family and the row, whole; `mod.rs` 1-257 except `Inventory` (this tree passes the worn items). **Left:**
`character.rs` 1-744 and the instance half of `instance.rs` (169-230, 422, 733-904).

**One deviation, the owner's:** what a skill does is a Lua script (`docs/decisions/0016-skills-are-lua-scripts.md`).
The script queues ASURA's operations; the engine runs them unchanged.

| ASURA | this tree | verdict |
|---|---|---|
| `skill/model.rs`, `skill/engine.rs` | `game/Skills/Model.rs`, `Engine.rs` | ported whole; `SkillDef::script` and the script seam (`CastView`, `SkillScripts`, `ops_for`) added and marked `KRYTA:` |
| `skill/tests.rs` (26 tests with the rules') | `game/Skills/Tests.rs` | ported; 26 pass, plus 3 for the seam (scripted ops run, a fault completes doing nothing, no host runs `effects`) |
| `skill/catalog.rs` | `game/Skills/Catalog.rs` | ported; reads `script`, `effects` optional beside it; `current()` not carried (read at start-up, rule 7) |
| `asura-rules/src/skills.rs` | `game/Skills/Rules.rs` | ported whole |
| `skill/wire.rs` | `game/Skills/Wire.rs` | ported; ASURA's `game::` builders are `AgentPackets`, the tuple builders' frame taken |
| `asura-protocol/src/game/skill.rs` (builders, readers, 4 tests) | `game/Server/Packets/SkillPackets.rs` | ported whole; headers from ASURA's table |
| `skill/mod.rs` (`Combat`, `deliveries`, `equipment`, `player_spec`, `npc_spec`, its test) | `game/Skills/mod.rs` | ported; `Loadout` (`character.rs:20-60`) moved here with the row |
| `asura-db/src/skills.rs`, migration `0034` | `Characters/Repository.rs` (`SkillProfile`, `skill_profile`, `record_skill_profile`), `data/sql/base/db_characters/0013-character-skills.sql` | ported; arrays cross the worker as comma text |
| `config/World/Vanilla/{skills,combat,attributes}.json` | same paths + `skills/*.lua` (44) | ported as data; each `effects` list is now its script, translated one-to-one |
| — | `game/Scripting/SkillScripts.rs` (mlua 0.10, Lua 5.4) | **ours, by decision 0016**: the host, sandbox, budget, 3 tests |
| `skill/character.rs` (load block: bar `0x00DA`, learned `0x00DB`, attributes `0x003A`, points `0x0037`/`0x0038`/`0x0039`, vitals; the nine requests `0x8026`, `0x8027`, `0x8046`, `0x805C`-`0x805F`, `0x800E`-`0x8010`; `/skill`) | `game/Characters/Skills.rs` (the load numbers), `game/Handlers/SkillHandler.rs` (the nine requests, `/skill`), `WorldSession::load_map` (the load block's frames), `Agents/Player.rs::load_block` | **ported** (2026-11-23): the load block carries the character's own attribute points, bar, learned bitmap, attribute snapshot and maxima; the nine requests answer with the authoritative state; `/skill` is a chat row (`config/Server/commands.json`) |
| `instance.rs` combat half (one `Combat` per copy, NPCs joined, the tick, `deliveries` to peers) | `game/Districts/Combat.rs` (`CombatDistricts`), `World::combat_join`/`combat_tick`/`combat_*`, the session tick | **ported** (2026-11-23), with the `0015` clock: the first connection to reach the period ticks the copy, NPC combat blocks seed the engine and the brain, deliveries go to the owner or the whole copy |
| `npc/fight.rs` | `game/Combat/mod.rs` | **ported** (2026-11-23): the state machine, the aggro/assist/leash numbers from `creatures.json`, the walk orders applied through the NPC table's path finder, `Fall`/`Restore`/`Revive` on the engine |
| `approach.rs` | `game/Movement/Approach.rs` (carried with the vendor subject, 7j) | ported; an attack request does not yet start a walk - the client walks itself into reach, and the engine swings once it is there |

**Compiled and tested (2026-11-23):** the whole workspace builds; `cargo test -p game --lib` is 483 passed, 0
failed, 16 ignored, including the 26 engine tests, the 3 script-seam tests, the rules, the packet family's 4, the
fight's 9 and the delivery test. **Not proved by a client:** the cast's message numbers and order and the fight's
aggro/leash/respawn values remain preforged's and chosen numbers, not a capture; the numbers are named in 7i and
in `docs/game-rules.md`.

## 7j. Vendors, collectors and Xunlai storage, from the local ASURA tree

**Opened (2026-11-23):** `crates/asura-world/src/vendor/mod.rs` (924 lines), `vendor/rules.rs` (797),
`vendor/content.rs` (478), `crates/asura-protocol/src/game/vendor.rs` (445),
`crates/asura-world/src/approach.rs` (468), `instance.rs:885-961` (`steer_approaches`),
`item.rs:25-100` and `:356-401` (the purse and `commit_ops`'s `extra`), `asura-db` migration `0035` and
`inventory.rs:1090-1197`, and `config/World/Vanilla/vendors.json`. **Carried:** `vendor.rs` 1-445,
`rules.rs` 1-797 and `content.rs` 23-378 whole with their tests (the `0x804A`/`0x804D` reader, every
builder, the price, sale, exchange and gold-move rules), `approach.rs` 27-320 and its 7 tests,
`vendor/mod.rs` 37-199, 218-255, 291-388, 390-721. The 18 rule and protocol tests and the 7 walk
tests were run against this tree's own `Agent` outside the workspace on 2026-11-23; the workspace
itself is not yet built with them. **Client proof pending.**

| ASURA | this tree | verdict |
|---|---|---|
| `asura-protocol/src/game/vendor.rs`, and `game/mod.rs:2311-2336` (`0x002A`) | `game/Server/Packets/VendorPackets.rs` | ported, with every test |
| `vendor/rules.rs` | `game/Trader/Rules.rs` | ported, with every test |
| `vendor/content.rs` | `game/Trader/Content.rs` (the types, `check`, `service`, `limits`) and `game/Maps/VendorPack.rs` (the file) | ported; the file is read with this tree's JSON and an unknown key is refused. The source is to become Lua (`docs/decisions/0017-vendor-content-is-the-packs.md`); a file that does not check refuses the start, where ASURA logs it and opens nothing |
| `config/World/Vanilla/vendors.json` | `config/World/Vanilla/vendors.json` | ported as data, unchanged |
| `asura-protocol/src/game/dialog.rs` | `game/Server/Packets/DialogPackets.rs` | ported, with every test |
| `dialog.rs` (the reader, `When`, `Page::is_open`/`visible`/`has_quest_step`, `check`, `talk_range_for`) | `config/World/Vanilla/scripts/runtime.lua`, run by `game/Scripting/NpcScripts.rs` | **ours, by decision 0019**: the same pages, conditions, actions and checks, in Lua; a condition may be a function and a dialog may carry `on_talk`/`on_choose` |
| `config/World/Vanilla/dialogs.json` | `config/World/Vanilla/dialogs/<npc>.lua` (42 files) | ported as data: every unit, id, skill and page target checked against ASURA's file (pages numbered from 1) |
| `approach.rs` | `game/Movement/Approach.rs`; the requests in `Handlers/NpcHandler.rs` | **ported thin**: `Purpose::Attack` is never started here (the fight owns it). The dialog's talk range (`approach.rs:288-298`) is ported (2026-11-23), asked of the Lua runtime. The steer runs on each connection's own tick (`WorldSession`), as this tree's NPC clock does (decision 0015), and its frames go to the copy through `Peers::broadcast` |
| `dispatch.rs:44-66`, `:82-84`, `:222-229` | the arms in `WorldSession` for `0x803E`, `0x803D`, `0x8028` (forget the walk), `0x8039`, `0x8033`, `0x804D`, `0x804A`, `0x807C` | ported; `0x803B` (`SEND_DIALOG`, `dispatch.rs:230-231`) with the dialog subject (2026-11-23) |
| `vendor/mod.rs` - `talk`, `open_merchant`, `handle_transaction`, `taken`, `placed`, `handle_change_gold`, `storage_open` | `game/Handlers/NpcHandler.rs` | **ported thin**: the dialog half of a talk (`:256-290`), `handle_send_dialog` (`:723-900`) and `in_reach`'s talk range (`:88-107`) are ported (2026-11-23), with the page, the choices and the action asked of the Lua runtime (decision 0019). A quest choice's step (`quest::handle_offer`, `handle_reward`, `handle_credit`) and the quest marker refresh (`:201-216`) wait for `quest.rs`; the `/xunlai` test command (`:902-923`) is not carried. The window's state, the walk and the purse are fields of `WorldSession` (`vendor`, `approach`, `purse`) |
| `item.rs:371-374` - the rules' `storage_open` from the window | `WorldSession::wearer_for`, every `ItemHandler` operation | ported: an item moves into or out of storage only while a Xunlai window is open and in reach |
| `item.rs:356-401` - `commit_ops`'s `extra` | `Characters/ItemOps.rs`, `commit_ops_with` | ported |
| `0035_storage_gold.sql`, `inventory.rs:1090-1197` | `data/sql/base/db_characters/0014-storage-gold.sql`; `RowChange::Gold`, `RowChange::StorageGold`, the sheet's `gold_storage` | **ported thin**: the write is guarded on the old balance as ASURA's is, but this tree's transaction does not yet refuse a statement that touched no row, so a stale balance writes nothing instead of rolling the operation back |

What ASURA itself does not do is not done here either: no live refusal is answered, and preforged
answers none.

## 7k. Death, the death penalty and the party wipe

**Opened (2026-11-23):** `docs/game-rules.md` E94-E114, Rurik's `studies/morale/FINDINGS.md` 1-3 and
7.1, `recordings/client/feature/anything/maps-live-20261123-023840.jsonl` seq 712-719, 721, 756,
780-790, and the ASURA engine's `die`/`resurrect` (`crates/asura-world/src/skill/engine.rs`, now
`game/Skills/Engine.rs`). **What was taken from ASURA:** the two events and their places in the tick;
the health/energy pools and the flags word. **What ASURA did not carry, and this tree now does:** the
death penalty, the measured death batch and the party wipe's clock and rise.

| subject | source | this tree | verdict |
|---|---|---|---|
| the death batch's order | E95; Rurik `studies/morale/FINDINGS.md` 1.3 (eleven of twelve player deaths): status word, morale pair, hold, strips, maxima, `0x002D`, `0x0026 {4}` | `game/Skills/Engine.rs` `die` and `Event::Death`; `game/Skills/Wire.rs`'s player death arm; the 16 (18 with a condition) status word | ported |
| the penalty's arithmetic | E96-E99; `FINDINGS.md` 2.1-2.2: `total + base * (morale - 100) / 100`, -15 a death, floor 40, energy on its base (25 total / 20 base -> 22) | `Actor::max_health`/`max_energy`, `effective`, `Engine::charged_deaths`; level-2/level-20 tests | ported |
| the free death | E100: fourteen seconds after a resurrection, and pre-Searing Ascalon never | `Actor::in_grace` (14 s), `World::death_charges` (the pack's `Ascalon (pre-Searing)` set: 145-151 and 160-166) | ported |
| the pre-Searing shape | this tree's own capture seq 712-719: status, hold, regeneration stops, `0x002D`, `0x0026 {4}` - no morale pair, no maxima | the same wire arm when `penalty == 0`; `a_free_death_sends_no_morale_pair_and_no_maxima` | ported |
| the wipe's clock | E106; `SLICE-F43`: `0x0180 [0x575, 5, 0, 10000]` three times 0.58 s after the last death | `World::death_tick`, `InstancePackets::instance_countdown`, `WorldSession`'s tick | ported |
| the rise | E103/E104: status, rescaled regeneration, hold release, then the refill one tick later; `0x017E` opens it | `Engine::resurrect` (`wipe: true`) and `Event::RiseRefill`; `Wire`'s two arms | ported |
| an ally's resurrection | E108: status, `0x00A2 [55, body, f]`, flags 9; no `0x017E` | `Engine::resurrect` (`wipe: false`), `Event::Resurrected`'s `fraction`; `Wire` | ported |
| the placement | E105/E114: the body jumps to the shrine, the walk records retire | `World::death_tick` sends `0x0025`/`0x002C`/`0x0028` and revives the engine body at the map's first spawn point | **ported thin**: the shrine coordinate is not content this world has (E114's "which shrine" is unanswered in `docs/game-rules.md`), so the entry point stands in; named here rather than invented |
| the penalty clears on an outpost load | E12/E113 | a new outpost is a new copy and a new actor, at morale 100 | ported |
| the penalty survives a field-to-field move | E12: both remain until the party enters an outpost | - | **missing**: a new copy's actors start at 100. The cross-zone morale is not carried by the player id yet |
| `/damage`, `/kill`, `/revive` | none - the engine's blows need a skill, and skills are still landing | `game/Chat/Operations.rs`, `Handlers/ChatHandler.rs`, `World::combat_hurt`/`combat_kill`/`combat_revive`, `config/Server/commands.json` | **ours, by the owner's word**: the test surface for the cycle before a skill lands the blow |
| the corpse and the fight | Rurik's 2026-09-30 symptom: a hostile holds aggro on a dead party through the countdown | - | **missing**: the brain drops a dead player from its target list (`combat/mod.rs` `think`), but no order is sent to a hostile whose bout ended with the party's last death |

**Not proved:** the whole cycle has unit tests (`a_player_death_sends_the_measured_batch_and_shrinks_the_pools`,
`a_free_death_sends_no_morale_pair_and_no_maxima`, `death_ends_effects_and_resurrection_brings_back_a_dead_ally`)
but no client run of the wipe and the shrine rise yet; the entry-point placement is the one difference a
client would see from E105.

## 8. The order this audit imposes

1. **(done)** The map's path graph: `map_data.rs` (the blob's own parse), `graph_dump.rs`,
   `path_tables.rs`, `pathfind.rs` and `pathfind/directional.rs`, on top of the archive - wired into the
   world and into the two handlers that ask for a walk. A walk is answered from the map's own graph, and
   a map with no graph is refused with `NoGraph`, the working implementation's own answer.
2. **(done)** The doorway crossing (`portals.rs`, whole) and the travel's cycle, so a step through a
   map's own wall starts a transfer that lands on the receiving map's arrival. What is not here is
   named in 7.4: ASURA's seat reservation, which is its own deployment accounting.
3. **(done)** The catalogue and the instance table's content: the world pack states every record ASURA's
   pack states (265 of them: 145 with a type and an archive file - the maps this world puts a client in -
   and 120 that state only an `instance_value`, which is a destination the client can resolve and this
   world cannot load), with 467 entry points, 28 arches and 28 arrivals from the same per-map files; an
   arrival names the map it comes from under the file's `from` key, and the receiving map owns it. A load
   sends **137** `{instance, value}` pairs in ten `0x01A0` batches. On this machine the world starts
   with `World of 265 map(s), 145 of them hosted` and loads the shipped path-graph cache. The cache is
   intentionally partial: each bound map can path, while a hosted or custom map without a graph reports
   `NoGraph`. `worldserver` never opens `Gw.dat`.

## 9. The world crate, file by file: what "ported" means here

`ASURA/crates/asura-world/src` is 36 files, 19,000-odd lines. Every one of them has a verdict, because
a claim that the port is done is only as good as the ledger under it. "Ported as data" means the file's
content is rows or a settings file here (rule 7's placement test: the world's data is rows, a setting
is a file); "this tree's arrangement" means the behaviour is here and the file's own shape is not,
because the reference's structure is this tree's authority for how a thing is arranged.

| ASURA file | lines | this tree | verdict |
|---|---|---|---|
| `default_equipment.rs` | 2,960 | `config/World/Vanilla/creation/equipment.json` | ported as data: 32 sets, 179 `ItemSpec`s, values checked against the file (map 0 / profession 1's first two items match field for field) |
| `pathfind.rs` | 1,794 | `game/Terrain/Pathfind.rs` | ported, and wired: a walk is answered from the map's own graph |
| `agent.rs` | 1,516 | `game/Agents/mod.rs`, the four handlers in `game/Handlers/MovementHandler.rs` | ported: `AgentState`, `Motion`, `Standing`, `densify`, the cadence, the tolerance and every handler |
| `map_data.rs` | 1,196 | `game/Terrain/MapData.rs` | ported whole, importer included |
| `lib.rs` | 1,164 | `game/World/`, `game/Agents/`, the tick | this tree's arrangement: `GameError` is an error string here, `mark_redirect`/`stamp_ms` are the redirect's own log line |
| `pvp_items.rs` | 1,567 | `game/DataStores/PvpItems.rs` | ported as data: the same 387 `(id, params)` rows, streamed in the load |
| `main.rs` | 727 | `worldserver/src/main.rs` | this tree's arrangement: the same stages in the reference's order, with the reference's stage numbers in the comments |
| `player.rs` | 540 | `game/Agents/Player.rs` (the load's three answers' frames), the session's unlocked bitmap and peer join | ported: `PlayerBlock`, `player_block`'s frame order, `unlock_bitmap`, `handle_request_players` |
| `connection.rs` | 587 | `game/Server/WorldSession.rs`, `WorldSocketMgr` | this tree's arrangement: one session per connection, the ping interval and value as settings |
| `travel.rs` | 534 | `answer_party_travel`, `begin_transfer`, `cross_portal`, `answer_teleport_command`, `answer_travel_init` - `game/Handlers/TravelHandler.rs` | ported; the seat reservation is named in 7.4 |
| `item.rs` | 561 at `ca9f532`; 1,298 today | `game/Handlers/ItemHandler.rs`, `game/Characters/{Inventory,ItemOps}.rs`, `game/Districts/Ground.rs` | **ported thin today**: the rules model, the five-step core, every client operation (move, equip, unequip, destroy, drop, pickup) and `/item`; `/item clear` and the load block's own diff remain (7h) |
| `instance.rs` | 528 | `game/Districts/` (`Districts`, `Peers`, `Instances`) | this tree's arrangement: a copy of a map is a district, and the population is `Peers` |
| `creation.rs` | 519 | `confirm_creation`, `answer_creation_*` in `game/Handlers/CharacterHandler.rs` | ported |
| `portals.rs` | 503 | `game/Maps/Portals.rs` | ported whole; the data is `map_arch` and `map_arrival` |
| `graph_dump.rs` | 473 | `game/Terrain/GraphDump.rs` | ported whole |
| `gwdat.rs` | 431 | `game/Pack/Gwdat.rs` | ported whole |
| `fa_compress.rs` | 402 | `game/Pack/FaCompress.rs` | ported whole |
| `chat.rs` | 354 | `game/Chat/`, `config/Server/commands.json`, and `answer_chat_message` + `answer_line` in `game/Handlers/ChatHandler.rs` | ported: the channel rule, `alias_key`; the command table is now **data** (`0011`); `/maps` uses a shared map-list function, with chat output unchanged. The three text frames are **not** ASURA's - they are the measured ones (`0012-chat-text-is-sent.md`) |
| `pathfind/directional.rs` | 320 | `game/Terrain/Pathfind/Directional.rs` | ported whole, its PROVISIONAL caveat kept |
| `replay.rs` | 302 | nothing | ASURA's own capture-replay harness; this tree's equivalent is `recordings/` and the `--ignored` end-to-end tests |
| `map_spawns.rs` | 264 | rows: `map_spawn` | ported as data (466 points, with the facing each states) |
| `instance_table.rs` | 223 | `game/Maps/InstanceTable.rs` | ported; the content is every `map` row with an `instance_value` |
| `cartography.rs` | 219 | `game/Maps/Cartography.rs` | ported: the grid's announcement and its slices |
| `dispatch.rs` | 218 | the dispatch in `WorldSession` | ported: 22 arms, 37 messages |
| `spawns.rs` | 197 | `PathGraphs` setting, `World.SpawnScatter` | ported as configuration |
| `capture.rs` | 195 | nothing | ASURA's own capture reader (see `replay.rs`) |
| `character_defaults.rs` | 186 | `World.Appearance` | ported as configuration |
| `travel_targets.rs` | 166 | `game/Maps/TravelTargets.rs` | ported: the twenty aliases |
| `catalogue.rs` | 153 | the `map` rows, `MapEntry::is_hostable` | ported as data and one predicate |
| `agent_defaults.rs` | 133 | `World.Agent*` settings | ported as configuration |
| `map.rs` | 113 | the four unasked messages of a load | ported |
| `world.rs` | 93 | `game/World/` | ported |
| `path_tables.rs` | 77 | `game/Terrain/PathTables.rs` | ported: both 256-entry tables, checked entry for entry |
| `fa_compress_tables.rs` | 69 | `game/Pack/FaCompressTables.rs` | ported: six static tables, value for value |
| `harness_off.rs` | 104 | nothing | a switch for ASURA's own harness (see `replay.rs`) |

Two crates beside it carry rules the login and the world both use:

| ASURA file | lines | this tree | verdict |
|---|---|---|---|
| `asura-rules/src/lib.rs` | 283 | `game/Maps/Placement.rs` | ported whole; its one datum is 7a's decision |
| `asura-auth/src/lib.rs` + `travel.rs` | 933 + 339 | `authserver/src/Server/` | ported: the 15 auth messages, the file check, the portal login, the travel record |

**The catalogue, measured on this machine before the headless separation.** `cargo test -p game --lib every_hosted_map -- --ignored --nocapture`
formerly imported 144 graphs out of `Gw.dat` and asked every hosted map with a spawn point to walk from one of its
own points to a nearby one: **136 of 138 walked**, none answered `NoGraph`, and the two that refused were
`119` (Augury Rock mission) and `642` (Eye of the North outpost). Both are `unplaced` records whose
`spawns.json` states no plane, so the row says plane 0 while the point lies on plane 21 and 22
(`trapezoid_at` finds it there) - and ASURA reads the same 0 (`map_spawns.rs:36-38`,
`#[serde(default)]`), so it refuses at that point too. A spawn file that omits its plane is a pack
defect, not a rule to invent around.

**An entry's facing is `(1, 0)` when its point states no angle.** `map_spawns.rs:44-53`: a point with no
`angle` keeps `(1, 0)`, which is the vector `GAME_SMSG_WORLD_CREATE_AGENT` carries. This tree faced the
neutral `+y` until round 18; both the spawn's own direction and the agent's create message now use the
point's stated angle, or `(1, 0)`.

**A map that arrived with the import loads through the whole protocol.**
`cargo test -p worldserver --test end_to_end -- --ignored a_second_outpost` takes **163 The Barradin
Estate** - not the map the schema seeded by hand - through a creation screen, R12's unlock written as a
scripted travel would, the login's travel row and the connection that presents it: the load is 1,180
bytes, `0x019A` names 163 with the explorable byte 1, the instance table closes it with 137 pairs in ten
batches, the ping pair is `0x000C` then `0x000D` carrying 20, and `0x0196` carries 163's own file
(380713) and a point within `World.SpawnScatter` of one of its own (R13).

**One thing a reader of the table should know.** The `0x01A0` and `0x008A` rows describe their bodies as
a `NestedStruct`, which the shared walker reads as sixteen-byte units while the frames carry a `u8`
count and six-byte pairs (or a compressed band). Those rows are ASURA's own and it emits the same
bytes; the walker is only ever asked to read a *server-emitted* frame in a test, so the tests read those
two blocks by size instead (`worldserver/tests/end_to_end.rs`).

**A reconnect resumes where the character left (R14).** `Position`, `position`, `record_position` and
`forget_position` are `ASURA/crates/asura-store/src/state/character.rs:213-294` ported onto the
character row (`data/sql/base/db_characters/0006-character-position.sql`: `map_id`, `pos_x`, `pos_y`,
`pos_plane`, where a NULL plane is "never saved"), and the entry rule is `agent.rs:478-545`: a doorway
landing first, an **outpost** rolls one of its own spawn points (R13), and every other map resumes the
saved position when it belongs to that map and stands on the map's own first point otherwise. The
position is written when the connection ends - not when a travel moved it, and never from the creation
instance, which would strand a character there (`connection.rs:405-423`) - and a doorway crossing writes
its landing while every other travel **clears** the position (`travel.rs:446-468`), because a travel
does not preserve where a character was. Only an outpost updates `last_outpost` (`player.rs:447`).
`a_reconnect_to_an_explorable_resumes_where_the_character_left` proves it end to end: map 43
Silverwood, a position reported and saved on disconnect, and the second connection standing at exactly
that point rather than the map's own first spawn.

**An arriving character unlocks the map it reaches; the server does not gate on it.** This was wrong
until 2026-11-20, and it is what a client saw as `007` when it walked through a portal: the step started
the crossing, the redirect went out, the client reconnected onto the map it had just walked into - and
this world **closed the connection** because that map was not in the character's unlocks. A fresh
character has one unlock, so a doorway always landed somewhere unlisted. The rule is
`ASURA/crates/asura-world/src/player.rs:442-471` - "The instance the character is standing in is
reached, so it is unlocked - that is the whole rule: the walk sets the bit, the bit never gates the
walk" - and it is also R12's own second sentence. The map a load enters is now written into
`character_unlock` when the players ask is answered, the bitmap the client receives includes it even if
the read fails (`player.rs:463-471`), and the creation instance sets nothing.
`a_step_through_ascalon_citys_arch_crosses_to_lakeside_county` proves the whole journey: the step
across Ascalon City's arch (148) starts the crossing, the redirect carries map 146 and a token, and the
connection that follows loads **Lakeside County (146)** standing on map 146's own arrival for a
character from 148 - `(6602.0, 4485.0)` - then unlocks it.

**The map's clock is ASURA's, and so is the moment a doorway is decided.** Two timing defects were
still in this tree until 2026-11-20, and together they are why a portal felt late:

- **The tick was driven by the socket's read timeout.** Each arriving message restarted that wait, so
  exactly the traffic the clock should pace is what starved it: while a client walked and talked, ticks
  were squeezed out, and the next one then carried the whole accumulated elapsed time - a clock that
  jumps. `ASURA/crates/asura-world/src/instance.rs:394-403` is a task per map on its own interval,
  independent of anything a client sends ("a map still ticks every interval, it just no longer owns a
  task that sleeps"). The deadline here is now **absolute** - one period from the tick that fired,
  whatever arrives in between.
- **The doorway was only tested on the client's reports.** ASURA decides it in the map's own step:
  `Instance::step` advances each agent and tests `before -> after` - the segment **its own copy walked**
  - and pushes the crossing to the connection, where the session's select serves it biased ahead of
  everything else (`instance.rs:290-308`, `connection.rs:494-502`). Here the clock now does the same
  (`WorldSession::step_map`): advance, test the segment, send the tick, then serve the transfer. A
  click-walk into an arch therefore crosses when the walk reaches it, not when a report happens to
  straddle the wall. The report path stays, because ASURA has it too (`dispatch.rs:44-54`).

The tick's default is also ASURA's own number now: **100 ms** (`ASURA/config/Server/intervals.json`,
`tick_interval_ms`, measured from the live client), where this tree had 105.

**What is not ported, and why - all three are stops, not gaps:**

- **NPCs and monsters.** ASURA creates exactly one agent per connection - the player's own
  (`agent.rs:1219`, `handle_request_spawn`; `world_create_agent` has one caller). The agent-creation
  stream for other bodies is not in the reference's capture for this build, so neither implementation
  sends one (`docs/port/07-manual-test.md`, "What is not expected to work yet").
- **ASURA's seat reservation** (`travel.rs:419-431`, `placement::Placement::reserve`): its own
  deployment accounting - a world registry, leases and a heartbeat - which is the multi-process
  arrangement this tree's structure does not have. Its capacity counterpart here is the district
  allotment (R5/R22/R58).
- **ASURA's harness** (`replay.rs`, `capture.rs`, `harness_off.rs`): tools for replaying its own
  captures, with no counterpart to port - the recordings themselves are in `recordings/`.
