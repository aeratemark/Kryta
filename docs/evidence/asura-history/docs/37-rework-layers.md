# Layers, from the code

This file lists the layers this tree actually compiles, read from `crates/` on 2026-11-06:
**10 crates, 81 source files, 34,863 lines**. It replaces the earlier version of this file, which
counted 92 files across 9 crates and described an `asura-config` crate that did not exist yet.

A layer here is one crate, and inside `asura-world` one module group. A crate is a layer because it
compiles alone and can be changed, tested and reverted alone.

## The crates, in dependency order

| # | crate | files | lines | depends on | what the code says it holds |
|---|---|---|---|---|---|
| 1 | `asura-protocol` | 14 | 7,720 | - | `lib.rs`: message layouts and framing. `packets.rs`: every packet in one table |
| 2 | `asura-db` | 11 | 3,543 | - | `lib.rs`: schema and migrations. `placement.rs`: registry, reservations, leases |
| 3 | `asura-config` | 3 | 885 | protocol | the numbers this machine runs at, and the files that hold them |
| 4 | `asura-rules` | 1 | 263 | protocol | game rules as decisions made before a world exists |
| 5 | `asura-auth` | 2 | 1,170 | protocol, db, rules, config | the login conversation, and travel placement |
| 6 | `asura-gateway` | 3 | 517 | protocol, db, auth, rules, config | one listener, one decision per connection, and the relay |
| 7 | `asura-webgate` | 2 | 631 | db | the four HTTP answers the client reads before login |
| 8 | `asura-world` | 35 | 17,945 | protocol, db, rules, config, auth, gateway | the world server: sessions, simulation, travel, content readers |
| 9 | `asura-supervisor` | 9 | 1,882 | db | starts this machine's services, watches them, reports |
| 10 | `asura-fakeworld` | 1 | 307 | db | a world server with no simulation, used to prove placement |

Two dependencies in that table are wider than a layer needs: `asura-world` names `asura-auth` and
`asura-gateway`, and `asura-config` names `asura-protocol` for one type (`MapKinds`).

## `asura-world`, grouped by module doc line

| group | files | what the code says |
|---|---|---|
| connection and session | `lib.rs`, `connection.rs`, `dispatch.rs`, `world.rs` | one gameplay connection: hello, load, gameplay, goodbye |
| instance and simulation | `instance.rs`, `agent.rs`, `player.rs`, `pathfind.rs`, `pathfind/directional.rs` | one hosted map; one agent's position and the rules that believe it |
| travel | `travel.rs`, `portals.rs`, `map_spawns.rs`, `travel_targets.rs` | leaving an instance; the pack's doorways and spawn lists |
| load and wire building | `map.rs`, `instance_table.rs`, `cartography.rs`, `catalogue.rs` | the load blocks, the instance table, the exploration grid |
| archive and map data | `gwdat.rs`, `map_data.rs`, `fa_compress.rs`, `graph_dump.rs` | how a `Gw.dat` entry becomes bytes and a path graph |
| content compiled in Rust | `default_equipment.rs` (2,957), `pvp_items.rs` (1,566), `path_tables.rs`, `fa_compress_tables.rs`, `travel_targets.rs` | generated tables, "do not edit by hand" |
| character and items | `creation.rs`, `character_defaults.rs`, `spawns.rs`, `item.rs`, `chat.rs` | creation choices, starting spawn, bags, commands |
| harness | `capture.rs`, `replay.rs`, `harness_off.rs` | instruments, off unless the `harness` feature is on |
| entry point | `main.rs`, `build.rs` | flags, startup order, build identity |

## Every file the code reads, and what it does when the file is absent

Two classes, and the difference is not written anywhere as a rule. It is what the code does.

**Falls back to a number compiled into the crate, and prints a log line:**

| file | reader | keys |
|---|---|---|
| `config/Server/intervals.json` | `asura-config/src/lib.rs:212` | tick, ping interval, ping value, probe timeout, freshness window, note |
| `config/Server/movement.json` | `asura-config/src/lib.rs:278` | `max_allowed_correction` |
| `config/Server/spawns.json` | `asura-config/src/lib.rs:354` | `spawn_scatter` |
| `config/Server/portals.json` | `asura-config/src/lib.rs:32` | `point_box_half_extent`; an invalid value disables point doorways |
| `config/Server/directional_movement.json` | `asura-config/src/directional.rs:17` | `distance_slack`, `edge_tolerance` |

**Refuses: a missing or malformed file is an error returned to the caller:**

| file | reader | keys |
|---|---|---|
| `config/Server/defaults.json` | `asura-config/src/lib.rs:631` | 7 keys, `default_outpost` required, no value in code |
| `config/Server/account_settings.json` | `asura-config/src/lib.rs:437` | the 591-byte login blob as hex |
| `config/Server/file_check.json` | `asura-config/src/lib.rs:521` | the 21-byte question, the 32-byte answer |
| `config/World/<pack>/character_defaults.json` | `asura-world/src/character_defaults.rs:122` | appearance |
| `config/World/<pack>/character_spawn.json` | `asura-world/src/spawns.rs:126` | one map per expansion source, plus a fallback |
| `config/World/<pack>/maps/**/basic.json` | `asura-config/src/world.rs:26`, `asura-world/src/instance_table.rs:101` | id, name, map type, blob id |
| `config/World/<pack>/maps/**/spawns.json` | `asura-world/src/map_spawns.rs:112` | the spawn list |
| `config/World/<pack>/maps/**/portals.json` | `asura-world/src/portals.rs:250` | `arches`, `arrivals` |
| `config/services.toml` | `asura-supervisor/src/config.rs:139` | what this machine runs |

Totals today: 8 JSON files and 2 TOML files under `config/Server` and `config/`, and 430 JSON files
under `config/` in all, of which 128 are map folders holding `basic.json`, `spawns.json`,
`portals.json`.

`.env` is a separate reader, `asura-db/src/config.rs:53`, and holds secrets only.

## Facts in the code that contradict text elsewhere

- `crates/asura-gateway/src/main.rs:88` names `config/Server/placement.json`. No such file exists;
  the fallback outpost is in `config/Server/defaults.json`.
- `crates/asura-world/src/map_spawns.rs:9` says the map still arrives from `t_map_config`;
  `crates/asura-config/src/world.rs` is what answers now.
- `crates/asura-db/src/game.rs` still holds five queries over `t_map_config`, `t_map_spawn` and
  `t_map_portal` (`:60`, `:73`, `:90`, `:103`, `:442`). No serving code calls them.
- `crates/asura-db/src/lib.rs:26-29` still tells an author to add a map as a row in `map_config`,
  while the map content is read from the pack.
- `config/World/README.md:14` says nothing reads the folder. `asura-config/src/world.rs` does.
- `immutable` in `asura-config/src/lib.rs:407` is read only to print a start-up line (`:646`). It
  changes no value and gates nothing.
