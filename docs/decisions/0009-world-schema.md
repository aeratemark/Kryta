# 0009 - The world schema: a map is a row, and so is where a character stands on it

Status: **superseded by `0014-the-world-content-is-a-pack.md`.** The world's map content is a pack of
JSON files under `config/World/<pack>/`, and the world reads that instead. The rows this record chose
remain in the schema and are no longer what the world serves.

## The question

The world needs maps, and a map needs a file id, a kind and the places a character may stand. Where does
that live - a JSON file like `data/maps.json` does today, or rows?

## The decision

Rows, in `kryta_world` (`data/sql/base/db_world/0001-world.sql`):

| table | holds | read by |
|---|---|---|
| `map` | id, type, file id, name, party cap, instance value | the travel answer (`0x0009`), the load (`0x0196`, `0x019A`, `0x01A0`), the party cap rule (R15) |
| `map_spawn` | one row per spawn point | entry placement (R13) |
| `map_arrival` | the point a portal crossing arrives at, per source map | portal arrivals (R21) |

And a map's rows are seeded by the updater from `data/sql/base/db_world/0002-maps.sql`, which carries one
outpost - Ascalon City (pre-Searing), map 148 - with the file id and spawn point a working server sent in
`recordings/asura/feature/travel/first-crossing-20261031.jsonl` seq 125.

The instance value is the one destination fact a map carries of its own: `0x0007-map-instance-value.sql`
adds the column and states map 148's `3138712889` (`0xBB14F539`), which is the pair a live load of it
carried (`recordings/client/feature/maps/pre-searing-load-3.jsonl` seq 751). A map whose row states no
value is left out of the `0x01A0` table and the world says so, because a wrong value is worse than a
destination the client cannot resolve yet.

## Why

1. **The test in `docs/port/04-configuration.md` decides it.** A map is the world's state - what exists,
   where - so it is rows; a setting is a choice an operator shares. `data/maps.json` was written before
   that test existed, and it is retired with this decision.
2. **A spawn is not a constant.** R13 has every entry pick a random point of the map's own list and land
   scattered around it, and spawn points are added and moved; a row is what moves.
3. **The messages already name the columns.** `0x0196` carries a map file id and a position; `0x019A`
   carries the map id and the district; the party cap is a rule, not a message, and it belongs on the map
   it applies to.

## What it costs

- The world program opens `kryta_world` and reads the tables at start-up, as the reference reads its map
  records, so a program started against an empty world database hosts no map and says so.
- `MapsFile` disappears from `worldserver.json`; the maps are the database's.

## What it does not decide

The instance tables - a running copy, its district, its population - and the travel record. Those come
with `game/Districts/` and the travel seam, and they are the other half of the world schema.
