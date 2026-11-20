-- Content is not seeded here. A map record, its entry points and its doorways are the world
-- pack's (`config/World/Vanilla/maps/<campaign>/<area>/<kind>/<map id>/`), read at start-up by
-- `game/src/Maps/Pack.rs`; a database row is for what a server writes and for nothing else
-- (`docs/decisions/0014-the-world-content-is-a-pack.md`, `docs/port/04-configuration.md`).

-- The map catalogue: every record the client can resolve, as the pack states it.
--
-- Imported from `ASURA/config/World/Vanilla/maps/**/basic.json` (265 folders): 145 state a
-- `map_type` and an archive file - the maps this world can put a client in - and 120 state only
-- an `instance_value`, which is a destination the client can name and this world cannot load.
-- `ASURA/crates/asura-config/src/world.rs:41-44` skips that second shape from its catalogue and
-- `ASURA/crates/asura-world/src/instance_table.rs:63-71` keeps it in the table; here both are
-- rows and `MapEntry::is_hostable` is the difference.
--
-- A record with no `map_file_id` gets 0, which is ASURA's own `unwrap_or(0)`
-- (`ASURA/crates/asura-world/src/catalogue.rs:60`); a record with no `name` is named by its id.

