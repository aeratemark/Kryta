//! The pack's NPC files, read: `npcs/definitions.json` and one `npcs.json` per map folder beside its
//! `spawns.json` and `portals.json` (ASURA `docs/49-npc-framework.md` section 3.2).
//!
//! Ported from `ASURA/crates/asura-world/src/npc/file.rs` (486 lines, the local repository as it
//! stands on 2026-11-22). Each item cites its ASURA line.
//!
//! This module only parses and walks; every check is [`NpcContent::assemble`]'s, so a test can build the
//! same content without a disk. Unlike the spawn lists, a bad NPC file is not skipped: one file that
//! does not parse refuses the whole pack, because a town with half its people is a second truth
//! (`file.rs:1-6`).
//!
//! Two differences of arrangement, not of behaviour:
//!
//! - **The compass range is this tree's setting, not a pack file.** ASURA reads `npcs/sight.json`
//!   (`file.rs:343-350`); here the same number is already `World.CompassRange` (10,000, rule R34,
//!   `game/World/WorldConfig.rs`), so [`load`] is handed the [`SightRules`] and a pack carries no second
//!   copy of it.
//! - **The rows are read with this tree's JSON, not serde derives.** ASURA's `deny_unknown_fields` is
//!   kept: a key a row does not know refuses the file. The four commentary names every file of this tree
//!   may carry (`doc`, `note`, `immutable`, `information`, `common/src/Configuration/Json.rs`) are walked
//!   past, as ASURA walks past its `note` and `fields`.
//!
//! It lives here, beside `Pack.rs`, and not in the NPC noun's folder: it reads the disk, and
//! `worldserver/tests/content_is_held.rs` refuses a file read inside a module that serves a
//! connection. `game/Agents/NpcFile.rs` re-exports it while its callers move.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::agents::npc_content::{
    Aggression, FourCc, MapRecord, NpcAttack, NpcCombat, NpcContent, NpcDefinition,
    PlacementRecord, Property, PropertyValue, Route, RouteMode, RoutePoint, SightRules,
};
use crate::server::packets::item_packets::NamedItem;

/// The pack-level folder that holds the NPC types (`file.rs:16-17`).
pub const DIR: &str = "npcs";
/// The NPC types, inside [`DIR`] (`file.rs:18-19`).
pub const DEFINITIONS: &str = "definitions.json";
/// One map's bodies and routes, inside the map's own folder (`file.rs:22-23`).
pub const MAP_FILE: &str = "npcs.json";
/// The pack folder the map folders are under (`ASURA/crates/asura-world/src/map_spawns.rs`, `MAPS`;
/// `game/Maps/Pack.rs`).
pub const MAPS: &str = "maps";

/// The names every file of this tree may carry as commentary (`common/src/Configuration/Json.rs`).
const COMMENTARY: [&str; 4] = ["doc", "note", "immutable", "information"];

/// One JSON object with the keys it may hold: anything else refuses it, as ASURA's
/// `#[serde(deny_unknown_fields)]` does on every row (`file.rs:26-170`).
struct Row<'a> {
    at: String,
    map: &'a Map<String, Value>,
}

impl<'a> Row<'a> {
    fn new(value: &'a Value, at: String, known: &[&str]) -> Result<Row<'a>, String> {
        let Some(map) = value.as_object() else {
            return Err(format!("{at}: is not an object"));
        };
        for key in map.keys() {
            if !known.contains(&key.as_str()) && !COMMENTARY.contains(&key.as_str()) {
                return Err(format!("{at}: unknown field {key:?}"));
            }
        }
        Ok(Row { at, map })
    }

    /// A field the row cannot be without.
    fn required<T: FromJson>(&self, key: &str) -> Result<T, String> {
        self.optional(key)?
            .ok_or_else(|| format!("{}: missing field {key:?}", self.at))
    }

    /// A field the row may leave out; `null` counts as left out, as serde's `Option` reads it.
    fn optional<T: FromJson>(&self, key: &str) -> Result<Option<T>, String> {
        match self.map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => T::from_json(value)
                .map(Some)
                .ok_or_else(|| format!("{}: field {key:?} is not {}", self.at, T::WHAT)),
        }
    }

    /// A field with a default when it is left out (`#[serde(default)]`).
    fn or<T: FromJson>(&self, key: &str, default: T) -> Result<T, String> {
        Ok(self.optional(key)?.unwrap_or(default))
    }

    /// A field's list of rows, empty when it is left out.
    fn rows(&self, key: &str) -> Result<&'a [Value], String> {
        match self.map.get(key) {
            None | Some(Value::Null) => Ok(&[]),
            Some(Value::Array(items)) => Ok(items),
            Some(_) => Err(format!("{}: field {key:?} is not a list", self.at)),
        }
    }
}

/// A value a field can hold, read the way serde reads it into the same Rust type.
trait FromJson: Sized {
    /// What the field should have been, for the refusal.
    const WHAT: &'static str;
    fn from_json(value: &Value) -> Option<Self>;
}

macro_rules! unsigned {
    ($($ty:ty),*) => {$(
        impl FromJson for $ty {
            const WHAT: &'static str = concat!("a ", stringify!($ty));
            fn from_json(value: &Value) -> Option<Self> {
                value.as_u64().and_then(|number| <$ty>::try_from(number).ok())
            }
        }
    )*};
}
unsigned!(u8, u16, u32, u64, usize);

impl FromJson for i32 {
    const WHAT: &'static str = "an i32";
    fn from_json(value: &Value) -> Option<Self> {
        value.as_i64().and_then(|number| i32::try_from(number).ok())
    }
}

impl FromJson for f32 {
    const WHAT: &'static str = "a number";
    fn from_json(value: &Value) -> Option<Self> {
        value.as_f64().map(|number| number as f32)
    }
}

impl FromJson for bool {
    const WHAT: &'static str = "true or false";
    fn from_json(value: &Value) -> Option<Self> {
        value.as_bool()
    }
}

impl FromJson for String {
    const WHAT: &'static str = "text";
    fn from_json(value: &Value) -> Option<Self> {
        value.as_str().map(str::to_string)
    }
}

impl<T: FromJson> FromJson for Vec<T> {
    const WHAT: &'static str = "a list";
    fn from_json(value: &Value) -> Option<Self> {
        value.as_array()?.iter().map(T::from_json).collect()
    }
}

/// `[lead, offhand]`, each an item index or `null` (`file.rs:100-101`).
impl FromJson for [Option<usize>; 2] {
    const WHAT: &'static str = "two item indices or nulls";
    fn from_json(value: &Value) -> Option<Self> {
        let [lead, offhand] = value.as_array()?.as_slice() else {
            return None;
        };
        let hand = |value: &Value| match value {
            Value::Null => Some(None),
            other => usize::from_json(other).map(Some),
        };
        Some([hand(lead)?, hand(offhand)?])
    }
}

/// `[dx, dy]` (`file.rs:84-87`).
impl FromJson for [f32; 2] {
    const WHAT: &'static str = "two numbers";
    fn from_json(value: &Value) -> Option<Self> {
        let [x, y] = value.as_array()?.as_slice() else {
            return None;
        };
        Some([f32::from_json(x)?, f32::from_json(y)?])
    }
}

/// The team code as four printable ASCII characters
/// (`ASURA/crates/asura-world/src/npc/content.rs:66-75`, `FourCc`'s `Deserialize`).
fn team(row: &Row<'_>) -> Result<FourCc, String> {
    let text: String = row.required("team")?;
    FourCc::parse(&text).ok_or_else(|| {
        format!(
            "{}: team {text:?} is not four printable ASCII characters",
            row.at
        )
    })
}

/// One definition (`ASURA/crates/asura-world/src/npc/content.rs:79-115`).
fn definition(value: &Value, at: String) -> Result<NpcDefinition, String> {
    let row = Row::new(
        value,
        at,
        &[
            "id",
            "definition_index",
            "file_id",
            "h000a",
            "visual",
            "h0012",
            "flags",
            "primary",
            "level",
            "name",
            "model_files",
            "h000b",
            "speed",
            "team",
            "combat",
        ],
    )?;
    let combat = match row.map.get("combat") {
        None | Some(Value::Null) => None,
        Some(value) => Some(combat(value, format!("{}.combat", row.at))?),
    };
    Ok(NpcDefinition {
        id: row.required("id")?,
        definition_index: row.required("definition_index")?,
        file_id: row.required("file_id")?,
        h000a: row.required("h000a")?,
        visual: row.required("visual")?,
        h0012: row.required("h0012")?,
        flags: row.required("flags")?,
        primary: row.required("primary")?,
        level: row.required("level")?,
        name: row.required("name")?,
        model_files: row.required("model_files")?,
        h000b: row.required("h000b")?,
        speed: row.required("speed")?,
        team: team(&row)?,
        combat,
    })
}

/// A combat block (`ASURA/crates/asura-world/src/npc/content.rs:121-168`).
fn combat(value: &Value, at: String) -> Result<NpcCombat, String> {
    let row = Row::new(
        value,
        at,
        &[
            "hostile",
            "stationary",
            "aggression",
            "health",
            "armor",
            "energy",
            "energy_regen",
            "rank",
            "profession",
            "attack",
            "skills",
        ],
    )?;
    let aggression = match row.optional::<String>("aggression")?.as_deref() {
        None | Some("always") => Aggression::Always,
        Some("when_provoked") => Aggression::WhenProvoked,
        Some(other) => {
            return Err(format!(
                "{}: aggression {other:?} is neither \"always\" nor \"when_provoked\"",
                row.at
            ));
        }
    };
    let attack = match row.map.get("attack") {
        None | Some(Value::Null) => None,
        Some(value) => Some(attack(value, format!("{}.attack", row.at))?),
    };
    Ok(NpcCombat {
        hostile: row.required("hostile")?,
        stationary: row.or("stationary", false)?,
        aggression,
        health: row.required("health")?,
        armor: row.required("armor")?,
        energy: row.or("energy", 0)?,
        energy_regen: row.or("energy_regen", 0)?,
        rank: row.or("rank", 0)?,
        profession: row.or("profession", 0)?,
        attack,
        skills: row.or("skills", Vec::new())?,
    })
}

/// A creature's attack (`ASURA/crates/asura-world/src/npc/content.rs:170-194`).
fn attack(value: &Value, at: String) -> Result<NpcAttack, String> {
    let row = Row::new(
        value,
        at,
        &[
            "item_type",
            "period_ms",
            "damage_type",
            "ranged",
            "min",
            "max",
            "projectile",
            "projectile_flag",
        ],
    )?;
    Ok(NpcAttack {
        item_type: row.or("item_type", 0)?,
        period_ms: row.required("period_ms")?,
        damage_type: row.required("damage_type")?,
        ranged: row.required("ranged")?,
        min: row.required("min")?,
        max: row.required("max")?,
        projectile: row.optional("projectile")?,
        projectile_flag: row.or("projectile_flag", 0)?,
    })
}

/// `npcs/definitions.json`: its `definitions`, beside the `fields` sentences for whoever opens it
/// (`file.rs:25-38`).
fn definitions_file(document: &Value, path: &Path) -> Result<Vec<NpcDefinition>, String> {
    let at = path.display().to_string();
    let file = Row::new(document, at.clone(), &["fields", "definitions"])?;
    let mut definitions = Vec::new();
    for (index, value) in file.rows("definitions")?.iter().enumerate() {
        definitions.push(definition(value, format!("{at}: definitions[{index}]"))?);
    }
    Ok(definitions)
}

/// One placement as a row states it (`file.rs:73-108`), and its properties and items still as rows.
struct PlacementRow {
    id: String,
    definition: String,
    x: f32,
    y: f32,
    plane: u16,
    angle: Option<f32>,
    direction: Option<[f32; 2]>,
    scale: Option<u32>,
    name: Option<Vec<u16>>,
    level: Option<u32>,
    effects: u32,
    h0023: f32,
    properties: Vec<(u32, Option<u32>, Option<f32>)>,
    items: Vec<NamedItem>,
    weapons: [Option<usize>; 2],
    route: Option<String>,
    walk_speed: Option<f32>,
    behavior: Option<String>,
}

/// One map's `npcs.json` (`file.rs:59-71`), with its routes' modes still as text.
struct MapFile {
    map_id: i32,
    placements: Vec<PlacementRow>,
    routes: Vec<(String, String, Vec<RoutePoint>)>,
}

fn placement_row(value: &Value, at: String) -> Result<PlacementRow, String> {
    let row = Row::new(
        value,
        at,
        &[
            "id",
            "definition",
            "x",
            "y",
            "plane",
            "angle",
            "direction",
            "scale",
            "name",
            "level",
            "effects",
            "h0023",
            "properties",
            "items",
            "weapons",
            "route",
            "walk_speed",
            "behavior",
        ],
    )?;
    let mut properties = Vec::new();
    for (index, value) in row.rows("properties")?.iter().enumerate() {
        // `file.rs:141-150`: an id and at most one each of `int` and `float`.
        let property = Row::new(
            value,
            format!("{}.properties[{index}]", row.at),
            &["id", "int", "float"],
        )?;
        properties.push((
            property.required("id")?,
            property.optional("int")?,
            property.optional("float")?,
        ));
    }
    let mut items = Vec::new();
    for (index, value) in row.rows("items")?.iter().enumerate() {
        // `file.rs:110-139`: the wire's item id is allocated by the world, so it is 0 here.
        let item = Row::new(
            value,
            format!("{}.items[{index}]", row.at),
            &[
                "file_id",
                "item_type",
                "dye_tint",
                "dye_colors",
                "materials",
                "unk1",
                "flags",
                "value",
                "model_id",
                "quantity",
                "name",
                "modifiers",
            ],
        )?;
        items.push(NamedItem {
            id: 0,
            file_id: item.required("file_id")?,
            item_type: item.required("item_type")?,
            dye_tint: item.or("dye_tint", 0)?,
            dye_colors: item.or("dye_colors", 0)?,
            materials: item.or("materials", 0)?,
            unk1: item.or("unk1", 0)?,
            flags: item.or("flags", 0)?,
            value: item.or("value", 0)?,
            model_id: item.required("model_id")?,
            quantity: item.or("quantity", 1)?,
            name: item.or("name", Vec::new())?,
            modifiers: item.or("modifiers", Vec::new())?,
        });
    }
    Ok(PlacementRow {
        id: row.required("id")?,
        definition: row.required("definition")?,
        x: row.required("x")?,
        y: row.required("y")?,
        plane: row.required("plane")?,
        angle: row.optional("angle")?,
        direction: row.optional("direction")?,
        scale: row.optional("scale")?,
        name: row.optional("name")?,
        level: row.optional("level")?,
        effects: row.required("effects")?,
        h0023: row.required("h0023")?,
        properties,
        items,
        weapons: row.or("weapons", [None, None])?,
        route: row.optional("route")?,
        walk_speed: row.optional("walk_speed")?,
        behavior: row.optional("behavior")?,
    })
}

fn map_file(document: &Value, path: &Path) -> Result<MapFile, String> {
    let at = path.display().to_string();
    let file = Row::new(document, at.clone(), &["map_id", "placements", "routes"])?;
    let mut placements = Vec::new();
    for (index, value) in file.rows("placements")?.iter().enumerate() {
        placements.push(placement_row(value, format!("{at}: placements[{index}]"))?);
    }
    let mut routes = Vec::new();
    for (index, value) in file.rows("routes")?.iter().enumerate() {
        // `file.rs:152-170`.
        let route = Row::new(
            value,
            format!("{at}: routes[{index}]"),
            &["id", "mode", "points"],
        )?;
        let mut points = Vec::new();
        for (point_index, value) in route.rows("points")?.iter().enumerate() {
            let point = Row::new(
                value,
                format!("{}.points[{point_index}]", route.at),
                &["x", "y", "plane", "pause_ms", "angle"],
            )?;
            points.push(RoutePoint {
                position: (point.required("x")?, point.required("y")?),
                plane: point.required("plane")?,
                pause_ms: point.or("pause_ms", 0)?,
                angle: point.optional("angle")?,
            });
        }
        if route.map.get("points").is_none() {
            return Err(format!("{}: missing field \"points\"", route.at));
        }
        routes.push((route.required("id")?, route.required("mode")?, points));
    }
    Ok(MapFile {
        map_id: file.required("map_id")?,
        placements,
        routes,
    })
}

/// A parsed map file as the content's record; a row that states its values inconsistently is a
/// problem here, as in ASURA (`file.rs:172-288`).
fn map_record(file: MapFile, problems: &mut Vec<String>) -> MapRecord {
    let map_id = file.map_id;
    let placements = file
        .placements
        .into_iter()
        .map(|row| {
            let properties = row
                .properties
                .iter()
                .filter_map(|&(id, int, float)| {
                    let value = match (int, float) {
                        (Some(value), None) => PropertyValue::Int(value),
                        (None, Some(value)) => PropertyValue::Float(value),
                        _ => {
                            problems.push(format!(
                                "map {map_id} placement {}: property {id} needs exactly one of int and float",
                                row.id
                            ));
                            return None;
                        }
                    };
                    Some(Property { id, value })
                })
                .collect();
            // Without either, `(1, 0)`, as a spawn point without an angle faces (`file.rs:199-211`).
            let direction = match (row.angle, row.direction) {
                (None, None) => (1.0, 0.0),
                (Some(angle), None) => (angle.cos(), angle.sin()),
                (None, Some([dx, dy])) => (dx, dy),
                (Some(_), Some(_)) => {
                    problems.push(format!(
                        "map {map_id} placement {}: give an angle or a direction, not both",
                        row.id
                    ));
                    (1.0, 0.0)
                }
            };
            PlacementRecord {
                id: row.id,
                definition: row.definition,
                position: (row.x, row.y),
                plane: row.plane,
                direction,
                scale: row.scale,
                name: row.name,
                level: row.level,
                effects: row.effects,
                h0023: row.h0023,
                properties,
                items: row.items,
                weapons: row.weapons,
                route: row.route,
                walk_speed: row.walk_speed,
                behavior: row.behavior,
            }
        })
        .collect();
    let routes = file
        .routes
        .into_iter()
        .map(|(id, mode, points)| {
            let mode = match mode.as_str() {
                "loop" => RouteMode::Loop,
                "once" => RouteMode::Once,
                "random" => RouteMode::Random,
                other => {
                    problems.push(format!(
                        "map {map_id} route {id}: mode {other:?} is neither \"loop\", \"once\" nor \
                         \"random\""
                    ));
                    RouteMode::Once
                }
            };
            Route { id, mode, points }
        })
        .collect();
    MapRecord {
        map_id,
        placements,
        routes,
    }
}

/// Read a JSON file, naming the file in the error (`file.rs:290-295`).
fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("{} is not readable: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

/// Every map folder's `npcs.json`, with its folder's map id. Only a folder named after a map id is a
/// map; the others are levels of the tree (`file.rs:297-326`, `game/Maps/Pack.rs`'s walk).
fn walk_maps(dir: &Path, found: &mut Vec<(i32, PathBuf)>, problems: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            problems.push(format!("{} is not readable: {err}", dir.display()));
            return;
        }
    };
    for entry in entries.flatten() {
        let folder = entry.path();
        if !folder.is_dir() {
            continue;
        }
        let name = folder
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        match name.parse::<i32>() {
            Ok(map_id) => {
                let file = folder.join(MAP_FILE);
                if file.exists() {
                    found.push((map_id, file));
                }
            }
            Err(_) => walk_maps(&folder, found, problems),
        }
    }
}

/// Read and check the pack's NPC content. A pack with no `npcs/` folder and no map file has no NPCs,
/// which is valid; a pack with either needs `npcs/definitions.json` (`file.rs:328-373`). The sight
/// rules are this tree's setting (see the module's note).
pub fn load(pack: &Path, sight: SightRules) -> Result<NpcContent, Vec<String>> {
    let mut problems = Vec::new();
    let mut files = Vec::new();
    let maps_dir = pack.join(MAPS);
    if maps_dir.is_dir() {
        walk_maps(&maps_dir, &mut files, &mut problems);
    }
    files.sort();
    let dir = pack.join(DIR);
    if !dir.is_dir() && files.is_empty() && problems.is_empty() {
        return Ok(NpcContent {
            sight,
            ..NpcContent::default()
        });
    }

    let path = dir.join(DEFINITIONS);
    let definitions = read(&path)
        .and_then(|document| definitions_file(&document, &path))
        .unwrap_or_else(|err| {
            problems.push(err);
            Vec::new()
        });
    let mut maps = Vec::new();
    for (folder_id, path) in files {
        match read(&path).and_then(|document| map_file(&document, &path)) {
            Ok(file) if file.map_id != folder_id => problems.push(format!(
                "{}: folder is {folder_id}, file says {}",
                path.display(),
                file.map_id
            )),
            Ok(file) => maps.push(map_record(file, &mut problems)),
            Err(err) => problems.push(err),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    NpcContent::assemble(sight, definitions, maps)
}

/// The start-up line's counts: definitions, placements and routes on so many maps
/// (`ASURA/crates/asura-world/src/npc/mod.rs:105-111`).
pub fn summary(content: &NpcContent) -> String {
    let (definitions, placements, routes) = content.counts();
    format!(
        "{definitions} NPC definition(s), {placements} placement(s) and {routes} route(s) on {} map(s)",
        content.maps.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sight() -> SightRules {
        SightRules {
            compass_range: 5000.0,
        }
    }

    fn pack(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join(DIR)).expect("create");
        dir
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("create");
        std::fs::write(path, text).expect("write");
    }

    const DEFINITIONS_JSON: &str = r#"{"definitions": [{"id": "guard", "definition_index": 1459,
        "file_id": 116227, "h000a": 0, "visual": 1677721600, "h0012": 0, "flags": 524, "primary": 1,
        "level": 2, "name": [3915, 35665, 58079, 18830], "model_files": [129945], "h000b": 9,
        "speed": 250.0, "team": "nonc"}]}"#;

    // `file.rs:416-424`.
    #[test]
    fn a_pack_without_npcs_has_none() {
        let dir = std::env::temp_dir().join("kryta-npc-none");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("maps").join("148")).expect("create");
        let content = load(&dir, sight()).expect("valid");
        assert_eq!(content.counts(), (0, 0, 0));
        assert_eq!(
            content.sight,
            sight(),
            "the setting is what the runtime reads"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // `file.rs:426-458`.
    #[test]
    fn a_map_file_is_read_from_its_folder_and_checked_against_it() {
        let dir = pack("kryta-npc-folder");
        write(&dir.join(DIR).join(DEFINITIONS), DEFINITIONS_JSON);
        let folder = dir.join("maps").join("Prophecies").join("Town").join("148");
        write(
            &folder.join(MAP_FILE),
            r#"{"map_id": 148, "placements": [{"id": "gate", "definition": "guard", "x": 1.0,
                "y": 2.0, "plane": 0, "effects": 0, "h0023": 1.0,
                "properties": [{"id": 11, "int": 5}]}]}"#,
        );
        let content = load(&dir, sight()).expect("loads");
        let map = content.map(148).expect("map");
        assert_eq!(map.placements[0].definition.team.text(), "nonc");
        assert_eq!(
            map.placements[0].properties,
            vec![Property {
                id: 11,
                value: PropertyValue::Int(5)
            }]
        );
        write(
            &folder.join(MAP_FILE),
            r#"{"map_id": 149, "placements": []}"#,
        );
        let problems = load(&dir, sight()).expect_err("refused");
        assert!(
            problems[0].contains("folder is 148, file says 149"),
            "{problems:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // `file.rs:460-485`.
    #[test]
    fn an_unknown_key_a_bad_team_and_a_two_valued_property_are_refused() {
        let dir = pack("kryta-npc-bad");
        write(
            &dir.join(DIR).join(DEFINITIONS),
            &DEFINITIONS_JSON.replace("\"nonc\"", "\"no\""),
        );
        write(
            &dir.join("maps").join("148").join(MAP_FILE),
            r#"{"map_id": 148, "placements": [{"id": "gate", "definition": "guard", "x": 1.0,
                "y": 2.0, "plane": 0, "effects": 0, "h0023": 1.0,
                "properties": [{"id": 11, "int": 5, "float": 1.0}]}]}"#,
        );
        let problems = load(&dir, sight()).expect_err("refused");
        let text = problems.join("\n");
        assert!(text.contains("team \"no\""), "{text}");
        assert!(text.contains("exactly one of int and float"), "{text}");
        write(
            &dir.join("maps").join("148").join(MAP_FILE),
            r#"{"map_id": 148, "placements": [], "colour": 1}"#,
        );
        let problems = load(&dir, sight()).expect_err("refused");
        assert!(problems.join("\n").contains("colour"), "{problems:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// This tree's commentary names are walked past, as ASURA walks past `note` and `fields`.
    #[test]
    fn commentary_is_not_a_field() {
        let dir = pack("kryta-npc-commentary");
        write(
            &dir.join(DIR).join(DEFINITIONS),
            &DEFINITIONS_JSON.replace(
                "{\"definitions\"",
                "{\"note\": \"n\", \"fields\": {\"id\": \"d\"}, \"information\": [1], \"definitions\"",
            ),
        );
        write(
            &dir.join("maps").join("148").join(MAP_FILE),
            r#"{"map_id": 148, "note": "n", "placements": [{"id": "gate", "definition": "guard",
                "x": 1.0, "y": 2.0, "plane": 0, "effects": 0, "h0023": 1.0, "doc": "seq 233",
                "direction": [0.5, 0.5], "weapons": [null, null]}]}"#,
        );
        let content = load(&dir, sight()).expect("loads");
        assert_eq!(
            content.map(148).expect("map").placements[0].direction,
            (0.5, 0.5)
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
