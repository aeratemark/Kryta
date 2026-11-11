//! The world: the maps it hosts, and what it loads at start-up.
//!
//! Reference: `src/server/game/World/World.{h,cpp}`, whose `SetInitialWorldSettings` is the stage
//! that loads everything the game is made of before the listener opens. This is that stage at the
//! size this tree has reached: a catalogue of the maps a world can put a client in, where a
//! character may stand on each, the creation screen's content, the numbers it runs by, and the
//! district copies every session shares.
//!
//! What it is not: the map manager. A map's own state arrives with the stage that can host one.

#[path = "WorldConfig.rs"]
pub mod world_config;

use common::logging::{Log, LogLevel};
use std::sync::{Arc, Mutex};

/// What kind of place a map is. The numbers are the client's, not this server's.
pub mod map_kind {
    /// A town or outpost.
    pub const OUTPOST: u8 = 0;
    /// An explorable area.
    pub const EXPLORABLE: u8 = 1;
    /// The character-creation instance: the one destination that is a conversation rather than a
    /// place, and the only one a world can serve without geometry.
    pub const CREATION: u8 = 11;
}

/// One map this world can put a client in.
#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    /// The client's own id for the map.
    pub id: u32,
    /// Which kind of place it is, from [`map_kind`].
    pub kind: u8,
    /// The `Gw.dat` file id the client loads the map from (`0x0196`).
    pub file_id: u32,
    /// What the map is called, for a log line and the administration tool.
    pub name: String,
}

impl MapEntry {
    /// Whether this entry is the character-creation instance.
    pub fn is_creation(&self) -> bool {
        self.kind == map_kind::CREATION
    }
}

/// The world a program represents.
#[derive(Debug, Default)]
pub struct World {
    realm: u32,
    maps: Vec<MapEntry>,
    /// The bags, items and profession sets a character-creation screen is built from.
    creation: crate::creation::Tables,
    /// Where a character may stand on entry, per map (R13).
    spawns: Vec<crate::maps::MapSpawn>,
    /// The numbers this world runs by: a district's allotment, a party's cap, the spawn scatter and the
    /// rest of the rules that name a number (docs/game-rules.md, R53).
    settings: world_config::WorldConfig,
    /// The running copies of maps: one outpost is one copy per district (R1, R4), and the district a
    /// character enters is this world's state, so every session shares it.
    districts: Arc<Mutex<crate::districts::Districts>>,
}

impl World {
    /// An empty world, before its settings are read.
    pub fn new(realm: u32) -> Self {
        Self {
            realm,
            maps: Vec::new(),
            creation: crate::creation::Tables::default(),
            spawns: Vec::new(),
            settings: world_config::WorldConfig::default(),
            districts: Arc::new(Mutex::new(crate::districts::Districts::new())),
        }
    }

    /// Which world this is: the id a login names as `map_token`.
    ///
    /// Reference: the reference's `realm.Id.Realm`, read from its own configuration as `RealmID`.
    pub fn realm(&self) -> u32 {
        self.realm
    }

    /// The maps this world hosts.
    pub fn maps(&self) -> &[MapEntry] {
        &self.maps
    }

    /// The numbers this world runs by.
    pub fn settings(&self) -> &world_config::WorldConfig {
        &self.settings
    }

    /// The running district copies, shared by every session.
    pub fn districts(&self) -> Arc<Mutex<crate::districts::Districts>> {
        Arc::clone(&self.districts)
    }

    /// Set the numbers this world runs by, after the program read its file.
    pub fn set_settings(&mut self, settings: world_config::WorldConfig) {
        self.settings = settings;
    }

    /// The creation data: the bags, the items and the profession sets.
    pub fn creation(&self) -> &crate::creation::Tables {
        &self.creation
    }

    /// Read the two creation files, and report what came from them.
    ///
    /// Reference: the same `SetInitialWorldSettings` pass that loads a world's content; a creation
    /// screen's bags and equipment are as much content as a map is.
    pub fn load_creation(
        &mut self,
        inventory_path: impl AsRef<std::path::Path>,
        equipment_path: impl AsRef<std::path::Path>,
        log: &Log,
    ) -> Result<(usize, usize, usize), String> {
        self.creation = crate::creation::Tables::load(inventory_path, equipment_path, log)?;
        Ok(self.creation.counts())
    }

    /// One map, by the client's id.
    pub fn map(&self, id: u32) -> Option<&MapEntry> {
        self.maps.iter().find(|map| map.id == id)
    }

    /// Read the maps, and where a character may stand on each, from the world database.
    ///
    /// Reference: the same `SetInitialWorldSettings` pass - a world that opens with no map hosts nothing,
    /// and says so rather than pretending.
    pub fn load_maps(&mut self, maps: &crate::maps::DatabaseMaps, log: &Log) -> Result<usize, String> {
        let (records, spawns) = maps.load()?;
        self.maps = records;
        self.spawns = spawns;
        log.out(
            LogLevel::Info,
            "server.worldserver",
            format!(
                "{} map(s) and {} spawn point(s) read from the world database.",
                self.maps.len(),
                self.spawns.len()
            ),
        );
        Ok(self.maps.len())
    }

    /// Give the world its maps and their spawn points directly: what a test does, and what a program
    /// that read them some other way would do.
    pub fn set_maps(&mut self, maps: Vec<MapEntry>, spawns: Vec<crate::maps::MapSpawn>) {
        self.maps = maps;
        self.spawns = spawns;
    }

    /// Every spawn point this world holds.
    pub fn spawns(&self) -> &[crate::maps::MapSpawn] {
        &self.spawns
    }

    /// Where a character may stand on entry, for one map (R13).
    pub fn spawns_of(&self, map_id: u32) -> Vec<crate::maps::MapSpawn> {
        self.spawns
            .iter()
            .filter(|spawn| spawn.map_id == map_id)
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world with two maps and one spawn point, as the database would give it.
    fn world() -> World {
        let mut world = World::new(2);
        world.set_maps(
            vec![
                MapEntry {
                    id: 0,
                    kind: map_kind::CREATION,
                    file_id: 0,
                    name: "Character creation".to_string(),
                },
                MapEntry {
                    id: 148,
                    kind: map_kind::OUTPOST,
                    file_id: 112_509,
                    name: "Ascalon City (pre-Searing)".to_string(),
                },
            ],
            vec![crate::maps::MapSpawn {
                map_id: 148,
                index: 0,
                x: 9541.87,
                y: 4796.46,
                plane: 0,
            }],
        );
        world
    }

    #[test]
    fn the_creation_instance_is_recognised_and_an_outpost_is_not() {
        let world = world();
        assert_eq!(world.realm(), 2);
        assert!(world.map(0).unwrap().is_creation());
        assert!(!world.map(148).unwrap().is_creation());
        assert!(world.map(999).is_none());
        assert_eq!(world.map(148).unwrap().file_id, 112_509);
    }

    #[test]
    fn a_map_carries_the_points_a_character_may_stand_on() {
        let world = world();
        let spawns = world.spawns_of(148);
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].x, 9541.87);
        assert!(world.spawns_of(0).is_empty(), "the creation instance has no point to stand on");
    }

    #[test]
    fn a_world_with_no_map_hosts_nothing() {
        let world = World::new(2);
        assert!(world.maps().is_empty());
        assert!(world.spawns_of(148).is_empty());
    }
}