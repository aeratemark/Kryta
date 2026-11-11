//! Where a world's maps come from: rows in `kryta_world` (`docs/decisions/0009-world-schema.md`).
//!
//! A map is the world's state - what exists, what file it loads from, where a character may stand - so it
//! is rows, not a settings file. The reference reads its map records the same way at
//! `SetInitialWorldSettings`: one query, held in memory for the life of the program, because a map's
//! definition does not change while the world serves.
//!
//! What is **not** here: an instance. A map is a definition; a running copy with a district, a population
//! and a clock is `game/Districts/`, and it is the half of the schema this repository does not read.

#[path = "Travels.rs"]
pub mod travels;

pub use travels::{DatabaseTravels, Travel};

use std::sync::{Arc, Mutex};

use common::logging::{Log, LogLevel};
use database::database::{DatabaseWorkerPool, Param};

use crate::world::MapEntry;

/// One place a character may stand on entry (R13).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapSpawn {
    /// The map's id.
    pub map_id: u32,
    /// Which point of that map's list this is.
    pub index: i16,
    /// The point, in the game's own units.
    pub x: f32,
    /// The point's other axis.
    pub y: f32,
    /// Which plane it is on.
    pub plane: i16,
}

const SELECT_MAPS: &str = "SELECT \"id\"::bigint, \"type\"::bigint, \"file_id\"::bigint, \"name\", \"party_cap\"::bigint FROM \"map\" ORDER BY \"id\"";

const SELECT_SPAWNS: &str = "SELECT \"map_id\"::bigint, \"idx\"::bigint, \"x\", \"y\", \"plane\"::bigint FROM \"map_spawn\" ORDER BY \"map_id\", \"idx\"";

/// The maps a world hosts, read from one database.
pub struct DatabaseMaps {
    pool: Arc<Mutex<DatabaseWorkerPool>>,
    log: Log,
}

impl DatabaseMaps {
    /// A repository that asks this pool.
    pub fn new(pool: Arc<Mutex<DatabaseWorkerPool>>, log: Log) -> Self {
        Self { pool, log }
    }

    /// Every map definition, and every spawn point.
    pub fn load(&self) -> Result<(Vec<MapEntry>, Vec<MapSpawn>), String> {
        let maps = self
            .query(SELECT_MAPS)?
            .iter()
            .map(|row| {
                Ok(MapEntry {
                    id: read_number(row, "id")? as u32,
                    kind: read_number(row, "type")? as u8,
                    file_id: read_number(row, "file_id")? as u32,
                    name: read_text(row, "name")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let spawns = self
            .query(SELECT_SPAWNS)?
            .iter()
            .map(|row| {
                Ok(MapSpawn {
                    map_id: read_number(row, "map_id")? as u32,
                    index: read_number(row, "idx")? as i16,
                    x: row
                        .try_get::<_, f32>("x")
                        .map_err(|error| format!("column x: {error}"))?,
                    y: row
                        .try_get::<_, f32>("y")
                        .map_err(|error| format!("column y: {error}"))?,
                    plane: read_number(row, "plane")? as i16,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok((maps, spawns))
    }

    /// Add a spawn point to a map at its next free index, and answer that index.
    ///
    /// The world's content is rows, and this is the administration tool's edit of one: the same
    /// `map_spawn` row the load's entry placement reads (R13). Every parameter is cast: the pool
    /// binds a number as 64-bit and these columns are narrower (`docs/decisions/0007-character-schema.md`).
    pub fn add_spawn(&self, map_id: u32, x: f32, y: f32, plane: i16) -> Result<i16, String> {
        let rows = self.query_with(
            "INSERT INTO \"map_spawn\" (\"map_id\", \"idx\", \"x\", \"y\", \"plane\") \
             SELECT $1::bigint::int, coalesce(max(\"idx\"), -1) + 1, $2::text::real, \
             $3::text::real, $4::bigint::int \
             FROM \"map_spawn\" WHERE \"map_id\" = $1::bigint::int RETURNING \"idx\"::bigint",
            vec![
                Param::Integer(i64::from(map_id)),
                Param::Text(format!("{x}")),
                Param::Text(format!("{y}")),
                Param::Integer(i64::from(plane)),
            ],
        )?;
        rows.first()
            .map(|row| read_number(row, "idx").map(|index| index as i16))
            .transpose()?
            .ok_or_else(|| "the spawn point was written but its index was not answered".to_string())
    }

    /// Remove a spawn point, and answer whether there was one at that index.
    pub fn remove_spawn(&self, map_id: u32, index: i16) -> Result<bool, String> {
        let rows = self.query_with(
            "DELETE FROM \"map_spawn\" WHERE \"map_id\" = $1::bigint::int AND \
             \"idx\" = $2::bigint::int RETURNING 1",
            vec![Param::Integer(i64::from(map_id)), Param::Integer(i64::from(index))],
        )?;
        Ok(!rows.is_empty())
    }

    /// One statement with parameters, and every row it answered.
    fn query_with(
        &self,
        sql: &str,
        params: Vec<database::database::Param>,
    ) -> Result<Vec<postgres::Row>, String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(sql.to_string(), params),
            Err(_) => return Err("the world database pool is poisoned".to_string()),
        };
        match reply.recv() {
            Ok(Ok(rows)) => Ok(rows),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.worldserver",
                    format!("the map tables could not be written: {error}"),
                );
                Err(error.to_string())
            }
            Err(_) => Err("the world database answered no query: it is shutting down".to_string()),
        }
    }

    /// One statement, and every row it answered.
    fn query(&self, sql: &str) -> Result<Vec<postgres::Row>, String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(sql, Vec::new()),
            Err(_) => return Err("the world database pool is poisoned".to_string()),
        };
        match reply.recv() {
            Ok(Ok(rows)) => Ok(rows),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.worldserver",
                    format!("the map tables could not be read: {error}"),
                );
                Err(error.to_string())
            }
            Err(_) => Err("the world database answered no query: it is shutting down".to_string()),
        }
    }
}

/// One integer column, read as the pool's own width.
fn read_number(row: &postgres::Row, column: &str) -> Result<i64, String> {
    row.try_get::<_, i64>(column)
        .map_err(|error| format!("column {column}: {error}"))
}

/// One text column.
fn read_text(row: &postgres::Row, column: &str) -> Result<String, String> {
    row.try_get::<_, String>(column)
        .map_err(|error| format!("column {column}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::configuration::{Config, Policy};
    use database::database::{read_settings, DatabaseKind};

    fn repository() -> Option<DatabaseMaps> {
        let log = Log::new();
        let config =
            Config::load_json("../worldserver/worldserver.json", false, Policy::default()).ok()?;
        let settings = read_settings(&config, DatabaseKind::World).ok()?;
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::World, &settings, &log).ok()?;
        pool.ping().ok()?;
        Some(DatabaseMaps::new(Arc::new(Mutex::new(pool)), log))
    }

    #[test]
    #[ignore = "needs the world database"]
    fn the_world_holds_the_outpost_it_seeded() {
        let Some(maps) = repository() else {
            panic!("the world database is not reachable");
        };
        let (records, spawns) = maps.load().expect("the map tables read");
        let outpost = records
            .iter()
            .find(|map| map.id == 148)
            .expect("map 148 is seeded");
        assert_eq!(outpost.kind, 0, "Ascalon City is an outpost");
        assert_eq!(outpost.file_id, 112_509, "the file id the recording carried");
        assert!(
            spawns.iter().any(|spawn| spawn.map_id == 148),
            "and it has a point to stand on"
        );
    }

    #[test]
    #[ignore = "needs the world database"]
    fn a_spawn_point_can_be_added_and_removed() {
        let Some(maps) = repository() else {
            panic!("the world database is not reachable");
        };
        let (_, before) = maps.load().expect("the map tables read");

        let index = maps
            .add_spawn(148, 9600.5, 4800.25, 0)
            .expect("the spawn point is written");
        let (_, spawns) = maps.load().expect("the map tables read");
        let added = spawns
            .iter()
            .find(|spawn| spawn.map_id == 148 && spawn.index == index)
            .expect("the point is the table's");
        assert!((added.x - 9600.5).abs() < 0.01 && (added.y - 4800.25).abs() < 0.01);

        assert!(maps.remove_spawn(148, index).expect("the spawn point is removed"));
        let (_, after) = maps.load().expect("the map tables read");
        assert_eq!(after.len(), before.len(), "the point is gone again");
    }
}
