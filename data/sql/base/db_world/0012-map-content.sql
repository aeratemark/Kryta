-- Content is not seeded here. A map record, its entry points and its doorways are the world
-- pack's (`config/World/Vanilla/maps/<campaign>/<area>/<kind>/<map id>/`), read at start-up by
-- `game/src/Maps/Pack.rs`; a database row is for what a server writes and for nothing else
-- (`docs/decisions/0014-the-world-content-is-a-pack.md`, `docs/port/04-configuration.md`).

-- A map's own content: where a character may stand on entry, the walls a step may cross, and
-- where an arrival from another map lands.
--
-- Imported from the pack's per-map folders (`ASURA/config/World/Vanilla/maps/**/spawns.json`,
-- `portals.json`): 466 spawn points with the facing each one states
-- (`ASURA/crates/asura-world/src/agent.rs:460-476`), 28 arches (a wall as two endpoints,
-- `portals.rs:79-120`) and 28 arrivals (`portals.rs:127-148`). A doorway's destination is the
-- receiving map's arrival for the map the character came from (R21).
--
-- A spawn row is numbered by its **position in the file**, which is what every `spawns.json` in
-- the pack states as `idx` except one: map 165 repeats `idx: 0` for its third point, and a row
-- key cannot repeat. The position is also what the random pick uses (`index = random % len`),
-- so a repeated label changes nothing a client sees.


