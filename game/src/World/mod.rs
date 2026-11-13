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
    /// Which kind of place it is, from [`map_kind`], or `None` for a record the client can resolve and
    /// this world does not host: the pack states a type only for a map it can put a client in
    /// (`ASURA/crates/asura-config/src/world.rs:41-44`, `catalogue::load`).
    pub kind: Option<u8>,
    /// The `Gw.dat` file id the client loads the map from (`0x0196`).
    pub file_id: u32,
    /// What the map is called, for a log line and the administration tool.
    pub name: String,
    /// The value the client resolves this map's destination by, in `0x01A0`. `None` means the map is
    /// left out of the instance table - a wrong value is worse than a destination the client cannot
    /// resolve yet (`ASURA/crates/asura-world/src/instance_table.rs:12-13`, `44-42`).
    pub instance_value: Option<u32>,
}

impl MapEntry {
    /// Whether this entry is the character-creation instance.
    pub fn is_creation(&self) -> bool {
        self.kind == Some(map_kind::CREATION)
    }

    /// Whether this world can put a client in this map at all.
    ///
    /// `ASURA/crates/asura-world/src/catalogue.rs:54-63`, `entry`: a record with no archive file is not
    /// hosted, **except** the character-creation instance, which is a conversation rather than a place
    /// and needs no geometry. A record with no type is not hosted either - it is a destination the
    /// client can name (`0x01A0`) and this world cannot load.
    pub fn is_hostable(&self) -> bool {
        match self.kind {
            None => false,
            Some(kind) => self.file_id != 0 || kind == map_kind::CREATION,
        }
    }

    /// How the map is named in an answer or a log line: its name when it has one, its id when it does
    /// not, and never an empty string.
    ///
    /// `ASURA/crates/asura-world/src/catalogue.rs:120-129`, `MapEntry::label`.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            format!("map {}", self.id)
        } else {
            format!("{} ({})", self.name, self.id)
        }
    }

    /// What kind of place it is, in words: `0` and `1` are the reference's own `MapType` values, and
    /// every other value it can carry is a kind of instance this world does not host.
    ///
    /// `ASURA/crates/asura-world/src/catalogue.rs:144-152`, `MapEntry::kind`.
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            Some(map_kind::OUTPOST) => "outpost",
            Some(map_kind::EXPLORABLE) => "explorable",
            Some(_) => "other",
            // A record with no type is a destination the client can resolve and this world cannot
            // serve (`ASURA/crates/asura-config/src/world.rs:41-44`).
            None => "destination only",
        }
    }

    /// The alias `/tp` would take for this map: the reference's aliases are lower-case names with
    /// single spaces ("lakeside county"), so this is the name lower-cased. An unnamed map has no alias
    /// to type and says so, rather than inventing one.
    ///
    /// `ASURA/crates/asura-world/src/catalogue.rs:134`.
    pub fn alias(&self) -> String {
        let alias = self.name.to_lowercase();
        let collapsed = alias.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            format!("map {}", self.id)
        } else {
            collapsed
        }
    }
}

/// The world a program represents.
#[derive(Default)]
pub struct World {
    realm: u32,
    maps: Vec<MapEntry>,
    /// The bags, items and profession sets a character-creation screen is built from.
    creation: crate::creation::Tables,
    /// Where a character may stand on entry, per map (R13).
    spawns: Vec<crate::maps::MapSpawn>,
    /// Where a portal crossing puts a character, per destination and source map (R21).
    arrivals: Vec<crate::maps::Arrival>,
    /// The numbers this world runs by: a district's allotment, a party's cap, the spawn scatter and the
    /// rest of the rules that name a number (docs/game-rules.md, R53).
    settings: world_config::WorldConfig,
    /// The running copies of maps: one outpost is one copy per district (R1, R4), and the district a
    /// character enters is this world's state, so every session shares it.
    districts: Arc<Mutex<crate::districts::Districts>>,
    /// The item templates the world database holds, read once at start-up: what a load resolves a
    /// character's held items against.
    templates: Vec<crate::data_stores::ItemDefinition>,
    /// The path graphs this world serves, one finder per map: a walk is answered from these, and a map
    /// with none answers `NoGraph` (`ASURA/crates/asura-world/src/pathfind.rs:1484`, `PathGraphs`).
    graphs: std::sync::Arc<crate::terrain::pathfind::PathGraphs>,
    /// The doorways this world judges a step against, one arch list per map, and the arrivals a
    /// crossing lands on (game/Maps/Portals.rs, ASURA/crates/asura-world/src/portals.rs).
    portals: std::sync::Arc<crate::maps::Portals>,
    /// The players loaded in this world's running copies, shared by every session: a copy of a map is a
    /// place other players can be in, and a connection is told about them (`game/Districts/Peers.rs`,
    /// `ASURA/crates/asura-world/src/instance.rs:406-473`).
    peers: Arc<Mutex<crate::districts::Peers>>,
}

impl World {
    /// An empty world, before its settings are read.
    pub fn new(realm: u32) -> Self {
        Self {
            realm,
            maps: Vec::new(),
            creation: crate::creation::Tables::default(),
            spawns: Vec::new(),
            arrivals: Vec::new(),
            settings: world_config::WorldConfig::default(),
            districts: Arc::new(Mutex::new(crate::districts::Districts::new())),
            templates: Vec::new(),
            graphs: std::sync::Arc::new(crate::terrain::pathfind::PathGraphs::empty()),
            portals: std::sync::Arc::new(crate::maps::Portals::empty()),
            peers: Arc::new(Mutex::new(crate::districts::Peers::new())),
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

    /// The item templates this world runs with.
    pub fn templates(&self) -> &[crate::data_stores::ItemDefinition] {
        &self.templates
    }

    /// The map's walkable graph, as a walk request is answered from.
    ///
    /// `ASURA/crates/asura-world/src/pathfind.rs:1484`, `PathGraphs`: one finder per map, held for the
    /// life of the program. A map whose graph was never bound answers `Walk::NoGraph` - the working
    /// implementation's own answer for a map it holds no graph for.
    pub fn graphs(&self) -> &std::sync::Arc<crate::terrain::pathfind::PathGraphs> {
        &self.graphs
    }

    /// The doorways this world judges a step against.
    ///
    /// `ASURA/crates/asura-world/src/portals.rs:313-377`: the pack's table, read once at start-up. A
    /// map the rows describe is judged by its own walls, and a step that crosses none of them crosses
    /// nothing.
    pub fn portals(&self) -> &std::sync::Arc<crate::maps::Portals> {
        &self.portals
    }

    /// Install the doorway table, once, before the listener opens.
    pub fn set_portals(&mut self, portals: crate::maps::Portals) {
        self.portals = std::sync::Arc::new(portals);
    }

    /// Install the graphs this world serves, once, before the listener opens.
    ///
    /// `ASURA/crates/asura-worldserver/src/main.rs:366-391`: the graphs are built before the world
    /// accepts anyone, from a dump when one is named and from the client archive otherwise, and every
    /// walk afterwards is answered from them.
    pub fn set_graphs(&mut self, graphs: crate::terrain::pathfind::PathGraphs) {
        self.graphs = std::sync::Arc::new(graphs);
    }

    /// The players loaded in this world's running copies.
    ///
    /// `ASURA/crates/asura-world/src/instance.rs:474-499`: the working implementation keeps one
    /// `Instance` per map and every session shares it. This tree's copy of a map is a district
    /// (`game/Districts/`), and the population is the same kind of shared state, so it lives beside it.
    pub fn peers(&self) -> Arc<Mutex<crate::districts::Peers>> {
        Arc::clone(&self.peers)
    }

    /// Set the item templates, after the program read them from its world database.
    pub fn set_templates(&mut self, templates: Vec<crate::data_stores::ItemDefinition>) {
        self.templates = templates;
    }

    /// The numbers this world runs by.
    pub fn settings(&self) -> &world_config::WorldConfig {
        &self.settings
    }

    /// The running district copies, shared by every session.
    pub fn districts(&self) -> Arc<Mutex<crate::districts::Districts>> {
        Arc::clone(&self.districts)
    }

    /// Give the copies their table and their log: what a program does once the world database is
    /// open and the maps are read (`docs/port/06-world-plan.md`, step 3).
    pub fn set_instances(
        &mut self,
        store: Arc<crate::districts::DatabaseInstances>,
        log: Log,
    ) {
        if let Ok(mut districts) = self.districts.lock() {
            districts.attach(store, log);
        }
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
        self.arrivals = maps.arrivals()?;
        log.out(
            LogLevel::Info,
            "server.worldserver",
            format!(
                "{} map(s), {} spawn point(s) and {} arrival point(s) read from the world database.",
                self.maps.len(),
                self.spawns.len(),
                self.arrivals.len()
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

    /// Every arrival point this world holds: where a portal crossing on a map lands (R21).
    pub fn arrivals(&self) -> &[crate::maps::Arrival] {
        &self.arrivals
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

    /// A map is named the way the working implementation names it: its name and its id, its id alone
    /// when it has no name, and never an empty string (`catalogue.rs:120-152`).
    #[test]
    fn a_map_is_named_as_the_working_implementation_names_it() {
        let named = MapEntry {
            id: 148,
            kind: Some(map_kind::OUTPOST),
            file_id: 113_021,
            name: "Ascalon City (pre-Searing)".to_string(),
            instance_value: Some(0xBB14_F539),
        };
        assert_eq!(named.label(), "Ascalon City (pre-Searing) (148)");
        assert_eq!(named.kind_name(), "outpost");
        let unnamed = MapEntry {
            id: 999,
            kind: Some(map_kind::EXPLORABLE),
            file_id: 0,
            name: String::new(),
            instance_value: None,
        };
        assert_eq!(unnamed.label(), "map 999", "an unnamed map names its id");
        assert_eq!(unnamed.kind_name(), "explorable");
        let other = MapEntry {
            id: 0,
            kind: Some(map_kind::CREATION),
            file_id: 0,
            name: "Character creation".to_string(),
            instance_value: None,
        };
        assert_eq!(other.kind_name(), "other", "a kind this world does not host");
    }

    /// A world with two maps and one spawn point, as the database would give it.
    fn world() -> World {
        let mut world = World::new(2);
        world.set_maps(
            vec![
                MapEntry {
                    id: 0,
                    kind: Some(map_kind::CREATION),
                    file_id: 0,
                    name: "Character creation".to_string(),
                    instance_value: None,
                },
                MapEntry {
                    id: 148,
                    kind: Some(map_kind::OUTPOST),
                    file_id: 112_509,
                    name: "Ascalon City (pre-Searing)".to_string(),
                    instance_value: Some(0xBB14_F539),
                },
            ],
            vec![crate::maps::MapSpawn {
                map_id: 148,
                index: 0,
                x: 9541.87,
                y: 4796.46,
                plane: 0,
                angle: None,
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

    /// `ASURA/crates/asura-world/src/catalogue.rs:54-63`, `entry`: a map with no archive file is not
    /// hosted, **except** the character-creation instance; and a record the pack states no type for is
    /// a destination the client can resolve (`0x01A0`) that this world cannot load.
    #[test]
    fn only_a_record_with_a_type_and_a_blob_is_hosted() {
        let host = MapEntry {
            id: 148,
            kind: Some(map_kind::OUTPOST),
            file_id: 113_021,
            name: "Ascalon City (pre-Searing)".to_string(),
            instance_value: Some(0xBB14_F539),
        };
        assert!(host.is_hostable());
        let creation = MapEntry {
            id: 0,
            kind: Some(map_kind::CREATION),
            file_id: 0,
            name: "Character Creation".to_string(),
            instance_value: None,
        };
        assert!(creation.is_hostable(), "the creation instance needs no geometry");
        let no_blob = MapEntry {
            file_id: 0,
            ..host.clone()
        };
        assert!(!no_blob.is_hostable(), "an outpost with no blob loads to a black screen");
        let destination_only = MapEntry {
            id: 4,
            kind: None,
            file_id: 0,
            name: String::new(),
            instance_value: Some(943_034_109),
        };
        assert!(
            !destination_only.is_hostable(),
            "a record with no type is a destination the client can resolve and this world cannot load"
        );
        assert_eq!(destination_only.kind_name(), "destination only");
        assert_eq!(destination_only.label(), "map 4", "an unnamed map names its id");
    }

    /// What the world program does at start-up: the graphs are imported for the maps it hosts, and a
    /// walk asked through the world is answered from them - map 148 from its own spawn point to a point
    /// beside it, and a map the archive holds no graph for with `NoGraph`.
    ///
    /// `ASURA/crates/asura-worldserver/src/main.rs:366-391`: the graphs are installed before the
    /// listener opens, and every walk after that is answered from them.
    #[test]
    fn a_world_installed_with_the_archives_graphs_answers_a_walk() {
        let Some(path) = std::env::var("KRYTA_GW_DAT").ok().or_else(|| {
            let candidate = std::path::PathBuf::from("Gw.dat");
            candidate
                .exists()
                .then(|| candidate.to_string_lossy().into_owned())
        }) else {
            eprintln!("skipped: no local Gw.dat");
            return;
        };
        let mut world = world();
        world.set_maps(
            vec![
                MapEntry {
                    id: 0,
                    kind: Some(map_kind::CREATION),
                    file_id: 0,
                    name: "Character creation".to_string(),
                    instance_value: None,
                },
                MapEntry {
                    id: 148,
                    kind: Some(map_kind::OUTPOST),
                    file_id: 0x1B97D,
                    name: "Ascalon City (pre-Searing)".to_string(),
                    instance_value: Some(0xBB14_F539),
                },
                MapEntry {
                    id: 146,
                    kind: Some(map_kind::OUTPOST),
                    file_id: 0x1,
                    name: "a map with no graph".to_string(),
                    instance_value: None,
                },
            ],
            world.spawns().to_vec(),
        );
        world.set_graphs(
            crate::terrain::pathfind::PathGraphs::load(&path, world.maps())
                .expect("the archive opens"),
        );
        assert_eq!(world.graphs().maps(), vec![148], "only the map with a blob is bound");
        let from = crate::terrain::Pos {
            x: 9541.87,
            y: 4796.46,
            plane: 0,
        };
        let to = crate::terrain::Pos {
            x: 9600.0,
            y: 4850.0,
            plane: 0,
        };
        assert!(
            matches!(world.graphs().walk(148, from, to), crate::terrain::Walk::Found(_)),
            "a bound map answers with a path"
        );
        assert!(
            matches!(world.graphs().walk(146, from, to), crate::terrain::Walk::NoGraph),
            "a map the archive holds no graph for answers NoGraph"
        );
    }
}