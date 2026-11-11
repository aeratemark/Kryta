//! The world: the maps it hosts, and what it loads at start-up.
//!
//! Reference: `src/server/game/World/World.{h,cpp}`, whose `SetInitialWorldSettings` is the stage
//! that loads everything the game is made of before the listener opens. This is that stage at the
//! size this tree has reached: a catalogue of the maps a world can put a client in, and nothing else
//! yet - no spawns, no items, no geometry.
//!
//! What it is not: the map manager. A map's own state arrives with the stage that can host one.

#[path = "WorldConfig.rs"]
pub mod world_config;

use common::configuration::json::Document;
use common::logging::{Log, LogLevel};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapEntry {
    /// The client's own id for the map.
    pub id: u32,
    /// Which kind of place it is, from [`map_kind`].
    pub kind: u8,
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
    /// The numbers this world runs by: a district's allotment, a party's cap, the spawn scatter and the
    /// rest of the rules that name a number (docs/game-rules.md, R53).
    settings: world_config::WorldConfig,
}

impl World {
    /// An empty world, before its settings are read.
    pub fn new(realm: u32) -> Self {
        Self {
            realm,
            maps: Vec::new(),
            creation: crate::creation::Tables::default(),
            settings: world_config::WorldConfig::default(),
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

    /// Load what the world is made of, and report it.
    ///
    /// Reference: `sWorld->SetInitialWorldSettings`, which loads maps, creatures, quests, items and
    /// the rest in one long sequence. Here it is one catalogue, read from the file the program names,
    /// and the log says how many maps came from it - zero is a statement, not a failure.
    pub fn load_maps(&mut self, path: impl AsRef<std::path::Path>, log: &Log) -> Result<usize, String> {
        let path = path.as_ref();
        if !path.exists() {
            log.out(
                LogLevel::Warn,
                "server.worldserver",
                format!("No map catalogue at {}: this world hosts no map.", path.display()),
            );
            return Ok(0);
        }

        let document = Document::read(path).map_err(|error| error.to_string())?;
        let records = document.records("maps")?;

        let mut maps = Vec::with_capacity(records.len());
        for record in records {
            let id: u32 = record
                .required("id")?
                .parse()
                .map_err(|_| format!("{}: 'id' is not a number", record.at()))?;
            let kind: u8 = record
                .required("type")?
                .parse()
                .map_err(|_| format!("{}: 'type' is not a number", record.at()))?;
            maps.push(MapEntry { id, kind });
        }

        self.maps = maps;
        Ok(self.maps.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, text: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kryta-world-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("maps.json");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn a_catalogue_is_read_and_the_creation_instance_is_recognised() {
        let log = Log::new();
        let mut world = World::new(2);
        let path = temp_file(
            "one",
            r#"{"maps": [{"id": 0, "type": 11}, {"id": 148, "type": 0}]}"#,
        );

        assert_eq!(world.load_maps(&path, &log).unwrap(), 2);
        assert_eq!(world.realm(), 2);
        assert!(world.map(0).unwrap().is_creation());
        assert!(!world.map(148).unwrap().is_creation());
        assert!(world.map(999).is_none());

        log.shutdown();
    }

    #[test]
    fn a_world_with_no_catalogue_hosts_nothing_and_says_so() {
        let log = Log::new();
        let mut world = World::new(2);
        let missing = std::env::temp_dir().join("kryta-world-nothing-maps.json");
        assert_eq!(world.load_maps(&missing, &log).unwrap(), 0);
        assert!(world.maps().is_empty());
        log.shutdown();
    }

    #[test]
    fn a_row_without_a_type_names_itself() {
        let log = Log::new();
        let mut world = World::new(2);
        let path = temp_file("bad", r#"{"maps": [{"id": 0}]}"#);
        let error = world.load_maps(&path, &log).unwrap_err();
        assert!(error.contains("maps[1]"), "{error}");
        log.shutdown();
    }
}
