//! Where a character's row comes from: the queries both programs read it with.
//!
//! The login's answer carries one `0x0007` per character of an account, and each of those is a row of
//! `kryta_characters.characters` read back (`docs/decisions/0007-character-schema.md`). The
//! administration tool lists every character the same way. A character is the player's, so it is a
//! row and not a file - the same line the account table sits on.
//!
//! The repository lives here rather than in the program that first used it, because two programs
//! read the same rows and one implementation of the data is the point (`AGENTS.md`, rule 11's move:
//! the auth program re-exports it).
//!
//! Reference: `AuthSrv_HandlePortalAccountLogin`, whose character list is a query of the character
//! database.

use std::sync::{Arc, Mutex};

use common::logging::{Log, LogLevel};
use database::database::{DatabaseWorkerPool, Param};

use crate::characters::{uuid_bytes, Character, CharacterSettings};
use crate::creation::Inventory;

/// The columns the list shows, in the order the list builds a character from them.
const SELECT_CHARACTERS: &str = "SELECT \"uuid\", \"name\", \"appearance\", \"primary_profession\", \
\"secondary_profession\", \"campaign\", \"level\", \"is_pvp\", \"helm_status\", \"last_outpost\" \
FROM \"characters\"";

/// The row an item a character owns is: everything the client is told about it, plus where it sits.
///
/// The definition lives on the row, not behind `template_id`: a load draws a character's items from
/// these rows alone, so an item cannot go missing because a template did
/// (`data/sql/base/db_characters/0005-item-instance-definition.sql`).
const INSERT_HELD_ITEM: &str = "INSERT INTO \"item_instance\" (\"character_uuid\", \"template_id\", \
\"file_id\", \"dye_tint\", \"dye_colors\", \"item_id\", \"bag_id\", \"slot\", \"quantity\", \
\"profession\", \"item_type\", \"model_id\", \"materials\", \"unk1\", \"flags\", \"value\", \
\"name\", \"modifiers\") \
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18) \
RETURNING 1";

const SELECT_HELD_ITEMS: &str = "SELECT \"file_id\"::bigint, \"dye_tint\"::bigint, \
\"dye_colors\"::bigint, \"item_id\"::bigint, \"bag_id\"::bigint, \"slot\"::bigint, \
\"quantity\"::bigint, \"profession\"::bigint, \"template_id\"::bigint, \"item_type\"::bigint, \
\"model_id\"::bigint, \"materials\"::bigint, \"unk1\"::bigint, \"flags\"::bigint, \
\"value\"::bigint, \"name\", \"modifiers\" FROM \"item_instance\" \
WHERE \"character_uuid\" = $1 ORDER BY \"bag_id\", \"slot\"";

/// One item a character holds: the whole item, and where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldItem {
    /// The `item_template` row the item was written from; 0 for an item no template describes.
    pub template_id: i64,
    /// The `Gw.dat` file id the client draws.
    pub file_id: u32,
    /// The item's kind code.
    pub item_type: u8,
    /// Which dye slot mask applies.
    pub dye_tint: u8,
    /// The four packed dye colours.
    pub dye_colors: u16,
    /// Crafting materials, as the reference packs them.
    pub materials: u16,
    /// The reference's own unverified byte.
    pub unk1: u8,
    /// Item flags.
    pub flags: u32,
    /// The item's value in gold.
    pub value: u32,
    /// The model id the client draws.
    pub model_id: u32,
    /// The runtime id the client keys the item by.
    pub item_id: u32,
    /// The bag it sits in.
    pub bag_id: u16,
    /// Its slot in that bag.
    pub slot: u8,
    /// How many are in the stack.
    pub quantity: u32,
    /// The profession the item was made for, or zero.
    pub profession: u8,
    /// The name, as UTF-16 code units.
    pub name: Vec<u16>,
    /// The modifier list, as dwords.
    pub modifiers: Vec<u32>,
}

/// The character database, as a program uses it.
pub struct DatabaseCharacters {
    pool: Arc<Mutex<DatabaseWorkerPool>>,
    log: Log,
}

impl DatabaseCharacters {
    /// A source that asks this pool.
    pub fn new(pool: Arc<Mutex<DatabaseWorkerPool>>, log: Log) -> Self {
        Self { pool, log }
    }

    /// Every character of the account the portal identified, read as the list needs them.
    ///
    /// A lookup that failed is reported and answers an empty list: a login whose character list cannot
    /// be read is a login with no character, and saying so plainly is better than failing the
    /// conversation the client is already in.
    pub fn for_account(&self, account_uuid: &str) -> Vec<Character> {
        self.read(&format!("{SELECT_CHARACTERS} WHERE \"account_uuid\" = $1 ORDER BY \"name\""),
            vec![Param::Text(account_uuid.to_string())])
    }

    /// Every character, for the administration tool's page: the rows as they are, in name order.
    pub fn list(&self) -> Vec<Character> {
        self.read(&format!("{SELECT_CHARACTERS} ORDER BY \"name\""), Vec::new())
    }

    /// Set a character's level - the row's own column, which a load reads back into the list and the
    /// character's own body (R46, R48).
    ///
    /// Reference: `CharacterDatabase`'s level update, which the reference's console reaches. The
    /// administration tool is the caller; the level's own range (1 to 20, R48) is the caller's check
    /// because it is a rule of the game rather than of the row. Answers whether the character row is
    /// there.
    pub fn set_level(&self, uuid: &str, level: u8) -> Result<bool, String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(
                "UPDATE \"characters\" SET \"level\" = $1 WHERE \"uuid\" = $2 RETURNING 1",
                vec![Param::Integer(i64::from(level)), Param::Text(uuid.to_string())],
            ),
            Err(_) => {
                return Err("the character database pool is poisoned; the level was not set"
                    .to_string());
            }
        };
        match reply.recv() {
            Ok(Ok(rows)) => Ok(!rows.is_empty()),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("Cannot set the level of {uuid}: {error}"),
                );
                Err(error)
            }
            Err(_) => Err("the character database answered no query: it is shutting down".to_string()),
        }
    }

    /// Record the outpost a character is now in - R14's "last outpost visited", which a later load
    /// (and the login's list) places it at.
    ///
    /// The row is updated through the worker queue like every other statement here. An update that
    /// failed is reported: the character is still in the map, but the world does not know it for a
    /// later reconnect.
    pub fn set_last_outpost(&self, uuid: &str, map_id: u16) -> Result<(), String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(
                "UPDATE \"characters\" SET \"last_outpost\" = $1 WHERE \"uuid\" = $2",
                vec![Param::Integer(i64::from(map_id)), Param::Text(uuid.to_string())],
            ),
            Err(_) => {
                return Err("the character database pool is poisoned; the outpost was not recorded"
                    .to_string());
            }
        };
        match reply.recv() {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("Cannot record the last outpost of {uuid}: {error}"),
                );
                Err(error)
            }
            Err(_) => Err("the character database answered no query: it is shutting down".to_string()),
        }
    }

    /// One query, and the characters it answered.
    fn read(&self, sql: &str, params: Vec<Param>) -> Vec<Character> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(sql.to_string(), params),
            Err(_) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    "the character database pool is poisoned; no character can be listed",
                );
                return Vec::new();
            }
        };

        match reply.recv() {
            Ok(Ok(rows)) => rows.iter().filter_map(|row| self.character(row)).collect(),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("Cannot read the characters: {error}"),
                );
                Vec::new()
            }
            Err(_) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    "the character database answered no query: it is shutting down",
                );
                Vec::new()
            }
        }
    }

    /// Write the items a new character starts with: one row per item, where it sits and which
    /// template it was written from.
    ///
    /// The list comes from the creation record set the screen drew (`Inventory`), which is a setting;
    /// what this writes is the character's own state - the rows the load then draws from. An item the
    /// templates do not describe is reported and not written: the client cannot be drawn an item the
    /// world does not know.
    pub fn write_starting_items(
        &self,
        uuid: &str,
        inventory: &Inventory,
        templates: &[crate::data_stores::ItemDefinition],
    ) -> Result<usize, String> {
        let mut written = 0usize;
        for item in &inventory.items {
            // The template is the row's key, not its content: everything the client is told lives on
            // the row, so an item no template describes is still written and still drawn. It is said
            // in the log because it means the world's template table and the creation record
            // disagree.
            let template_id = match templates.iter().find(|template| {
                template.file_id == item.file_id
                    && template.item_type == item.item_type
                    && template.dye_tint == item.dye_tint
                    && template.dye_colors == item.dye_colors
                    && template.materials == item.materials
                    && template.unk1 == item.unk1
                    && template.flags == item.flags
                    && template.value == item.value
                    && template.model_id == item.model_id
                    && template.quantity == item.quantity
                    && template.profession == item.profession
            }) {
                Some(template) => template.id,
                None => {
                    self.log.out(
                        LogLevel::Warn,
                        "server.characters",
                        format!(
                            "The creation set names item {} (file {}), which no item template describes; it is written from the record list alone.",
                            item.id, item.file_id
                        ),
                    );
                    0
                }
            };
            let reply = match self.pool.lock() {
                Ok(pool) => pool.query_reply(
                    INSERT_HELD_ITEM,
                    vec![
                        Param::Text(uuid.to_string()),
                        Param::Integer(template_id),
                        Param::Integer(i64::from(item.file_id)),
                        Param::Integer(i64::from(item.dye_tint)),
                        Param::Integer(i64::from(item.dye_colors)),
                        Param::Integer(i64::from(item.id)),
                        Param::Integer(i64::from(item.bag_id.unwrap_or(0))),
                        Param::Integer(i64::from(item.slot.unwrap_or(0))),
                        Param::Integer(i64::from(item.quantity)),
                        Param::Integer(i64::from(item.profession)),
                        Param::Integer(i64::from(item.item_type)),
                        Param::Integer(i64::from(item.model_id)),
                        Param::Integer(i64::from(item.materials)),
                        Param::Integer(i64::from(item.unk1)),
                        Param::Integer(i64::from(item.flags)),
                        Param::Integer(i64::from(item.value)),
                        Param::Text(units_text(&item.name)),
                        Param::Text(dwords_text(&item.modifiers)),
                    ],
                ),
                Err(_) => {
                    return Err(
                        "the character database pool is poisoned; the items were not written"
                            .to_string(),
                    );
                }
            };
            match reply.recv() {
                Ok(Ok(rows)) => written += rows.len(),
                Ok(Err(error)) => {
                    return Err(format!("an item row cannot be written: {error}"));
                }
                Err(_) => {
                    return Err(
                        "the character database answered no query: it is shutting down".to_string(),
                    );
                }
            }
        }
        Ok(written)
    }

    /// What a character holds, as its rows say: the definition's key and where each item sits.
    pub fn items_of(&self, uuid: &str) -> Result<Vec<HeldItem>, String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(SELECT_HELD_ITEMS, vec![Param::Text(uuid.to_string())]),
            Err(_) => return Err("the character database pool is poisoned".to_string()),
        };
        let rows = match reply.recv() {
            Ok(Ok(rows)) => rows,
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("Cannot read the items of {uuid}: {error}"),
                );
                return Err(error);
            }
            Err(_) => {
                return Err(
                    "the character database answered no query: it is shutting down".to_string(),
                );
            }
        };
        rows.iter()
            .map(|row| {
                let column = |at: usize| {
                    row.try_get::<usize, i64>(at)
                        .map_err(|error| format!("column {at}: {error}"))
                };
                let text = |at: usize| {
                    row.try_get::<usize, String>(at)
                        .map_err(|error| format!("column {at}: {error}"))
                };
                Ok(HeldItem {
                    file_id: column(0)? as u32,
                    dye_tint: column(1)? as u8,
                    dye_colors: column(2)? as u16,
                    item_id: column(3)? as u32,
                    bag_id: column(4)? as u16,
                    slot: column(5)? as u8,
                    quantity: column(6)? as u32,
                    profession: column(7)? as u8,
                    template_id: column(8)?,
                    item_type: column(9)? as u8,
                    model_id: column(10)? as u32,
                    materials: column(11)? as u16,
                    unk1: column(12)? as u8,
                    flags: column(13)? as u32,
                    value: column(14)? as u32,
                    name: units(&text(15)?),
                    modifiers: dwords(&text(16)?),
                })
            })
            .collect()
    }

    /// Move one of a character's items to a bag slot, and answer whether the row was the character's.
    ///
    /// The item's own row is the state: a drag that the client drew is this update, and the answer to
    /// the client is the placement it already knows (`0x013E`).
    pub fn move_item(
        &self,
        uuid: &str,
        item_id: u32,
        bag_id: u16,
        slot: u8,
    ) -> Result<bool, String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(
                "UPDATE \"item_instance\" SET \"bag_id\" = $1, \"slot\" = $2 \
                 WHERE \"character_uuid\" = $3 AND \"item_id\" = $4 RETURNING 1",
                vec![
                    Param::Integer(i64::from(bag_id)),
                    Param::Integer(i64::from(slot)),
                    Param::Text(uuid.to_string()),
                    Param::Integer(i64::from(item_id)),
                ],
            ),
            Err(_) => return Err("the character database pool is poisoned".to_string()),
        };
        match reply.recv() {
            Ok(Ok(rows)) => Ok(!rows.is_empty()),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("Cannot move item {item_id} of {uuid}: {error}"),
                );
                Err(error)
            }
            Err(_) => Err("the character database answered no query: it is shutting down".to_string()),
        }
    }

    /// Remove one of a character's items, and answer whether the row was the character's.
    ///
    /// The item's own row is the state: a destroy the client asked for is this delete, and the answer
    /// to the client is `0x013C`, the item gone.
    pub fn remove_item(&self, uuid: &str, item_id: u32) -> Result<bool, String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(
                "DELETE FROM \"item_instance\" WHERE \"character_uuid\" = $1 AND \"item_id\" = $2 \
                 RETURNING 1",
                vec![Param::Text(uuid.to_string()), Param::Integer(i64::from(item_id))],
            ),
            Err(_) => return Err("the character database pool is poisoned".to_string()),
        };
        match reply.recv() {
            Ok(Ok(rows)) => Ok(!rows.is_empty()),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("Cannot remove item {item_id} of {uuid}: {error}"),
                );
                Err(error)
            }
            Err(_) => Err("the character database answered no query: it is shutting down".to_string()),
        }
    }

    /// Remove a character and every record that is its own - its items, its unlocks, its row.
    ///
    /// Reference: `CharacterDatabase.DeleteFromDB`, which deletes the character and the records that
    /// point at it together. The administration tool is the caller; neither a login's list nor a
    /// load ever deletes a row. Answers whether the character's own row was there.
    pub fn remove(&self, uuid: &str) -> Result<bool, String> {
        let mut gone = false;
        for statement in [
            "DELETE FROM \"item_instance\" WHERE \"character_uuid\" = $1 RETURNING 1",
            "DELETE FROM \"character_unlock\" WHERE \"character_uuid\" = $1 RETURNING 1",
            "DELETE FROM \"characters\" WHERE \"uuid\" = $1 RETURNING 1",
        ] {
            let reply = match self.pool.lock() {
                Ok(pool) => pool.query_reply(statement, vec![Param::Text(uuid.to_string())]),
                Err(_) => return Err("the character database pool is poisoned".to_string()),
            };
            match reply.recv() {
                Ok(Ok(rows)) => {
                    if statement.contains("\"characters\"") {
                        gone = !rows.is_empty();
                    }
                }
                Ok(Err(error)) => {
                    self.log.out(
                        LogLevel::Error,
                        "server.characters",
                        format!("Cannot remove the records of {uuid}: {error}"),
                    );
                    return Err(error);
                }
                Err(_) => {
                    return Err(
                        "the character database answered no query: it is shutting down".to_string(),
                    );
                }
            }
        }
        Ok(gone)
    }

    /// Give a character one more map it may travel to (R12), answering whether the row was new.
    pub fn unlock_map(&self, uuid: &str, map_id: u32) -> Result<bool, String> {        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(
                "INSERT INTO \"character_unlock\" (\"character_uuid\", \"map_id\") \
                 VALUES ($1, $2::bigint) ON CONFLICT DO NOTHING RETURNING 1",
                vec![Param::Text(uuid.to_string()), Param::Integer(i64::from(map_id))],
            ),
            Err(_) => return Err("the character database pool is poisoned".to_string()),
        };
        match reply.recv() {
            Ok(Ok(rows)) => Ok(!rows.is_empty()),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("Cannot unlock map {map_id} for {uuid}: {error}"),
                );
                Err(error)
            }
            Err(_) => Err("the character database answered no query: it is shutting down".to_string()),
        }
    }

    /// The maps a character may travel to (R12), in id order.
    pub fn unlocked_maps(&self, uuid: &str) -> Result<Vec<u32>, String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(
                "SELECT \"map_id\"::bigint FROM \"character_unlock\" \
                 WHERE \"character_uuid\" = $1 ORDER BY \"map_id\"",
                vec![Param::Text(uuid.to_string())],
            ),
            Err(_) => return Err("the character database pool is poisoned".to_string()),
        };
        let rows = match reply.recv() {
            Ok(Ok(rows)) => rows,
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("Cannot read the unlocks of {uuid}: {error}"),
                );
                return Err(error);
            }
            Err(_) => {
                return Err(
                    "the character database answered no query: it is shutting down".to_string(),
                );
            }
        };
        Ok(rows.iter().map(|row| row.get::<usize, i64>(0) as u32).collect())
    }

    /// One row, as the list reads it.
    fn character(&self, row: &postgres::Row) -> Option<Character> {
        let uuid_text: String = row.get("uuid");
        let uuid = match uuid_bytes(&uuid_text) {
            Ok(uuid) => uuid,
            Err(error) => {
                self.log.out(
                    LogLevel::Error,
                    "server.characters",
                    format!("A character's uuid cannot be read: {error}"),
                );
                return None;
            }
        };

        // Every numeric column is BIGINT: a query parameter is a 64-bit integer in this pool, and a
        // narrower column would refuse it (docs/decisions/0007-character-schema.md).
        let appearance: i64 = row.get("appearance");
        let campaign: i64 = row.get("campaign");
        let level: i64 = row.get("level");
        let secondary: i64 = row.get("secondary_profession");
        let helm: i64 = row.get("helm_status");
        let outpost: i64 = row.get("last_outpost");

        Some(Character {
            uuid,
            name: row.get("name"),
            settings: CharacterSettings {
                last_outpost: outpost as u16,
                appearance: appearance as u32,
                campaign: campaign as u8,
                level: level as u8,
                is_pvp: row.get("is_pvp"),
                secondary_profession: secondary as u8,
                helm_status: helm as u8,
                // The armour pieces are the equipped bag's, and this tree has no items yet: a
                // character wearing nothing sends five empty slots, which is what the reference's
                // own record does.
                pieces: Vec::new(),
            },
        })
    }
}

/// A list of code units as comma-separated decimal text, the form the definition columns keep.
fn units_text(units: &[u16]) -> String {
    units
        .iter()
        .map(|unit| unit.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// A list of dwords as comma-separated decimal text.
fn dwords_text(values: &[u32]) -> String {
    values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// A column's code units, from the comma-separated decimal text it keeps.
fn units(text: &str) -> Vec<u16> {
    text.split(',')
        .filter(|part| !part.trim().is_empty())
        .filter_map(|part| part.trim().parse::<u16>().ok())
        .collect()
}

/// A column's dwords, from the comma-separated decimal text it keeps.
fn dwords(text: &str) -> Vec<u32> {
    text.split(',')
        .filter(|part| !part.trim().is_empty())
        .filter_map(|part| part.trim().parse::<u32>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::configuration::{Config, Policy};
    use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};

    /// The live character database, when this machine has one.
    ///
    /// A source that cannot open its pool answers `None`, so a checkout without a database skips the
    /// test rather than failing it - the same shape the database crate's own live tests have.
    fn source() -> Option<DatabaseCharacters> {
        let log = Log::new();
        let config =
            Config::load_json("../authserver/authserver.json", false, Policy::default()).ok()?;
        let settings = read_settings(&config, DatabaseKind::Character).ok()?;
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::Character, &settings, &log).ok()?;
        pool.ping().ok()?;
        Some(DatabaseCharacters::new(
            Arc::new(Mutex::new(pool)),
            log,
        ))
    }

    #[test]
    #[ignore = "needs the character database"]
    fn an_accounts_characters_are_listed_with_their_settings() {
        // Serialized with the other live tests that write the same table: this one counts one
        // account's rows, and another test's own row (on the same account) would be counted if it
        // interleaved.
        let _serial = database::updater::apply::test_lock("characters");
        let Some(source) = source() else {
            panic!("the character database is not reachable");
        };

        // A character of its own, so the test does not depend on what a client created.
        let account = "00000000-0000-0000-0000-00000000C10D";
        let uuid = "00000000-0000-0000-0000-0000000000AA";
        let pool = Arc::clone(&source.pool);
        {
            let mut pool = pool.lock().unwrap();
            pool.query_with(
                "DELETE FROM \"characters\" WHERE \"uuid\" = $1",
                &[&uuid],
            )
            .unwrap();
            pool.query_with(
                "INSERT INTO \"characters\" (\"uuid\", \"account_uuid\", \"name\", \"appearance\", \"primary_profession\", \"unlocked_professions\", \"campaign\", \"level\") \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[&uuid, &account, &"Listed One", &0xABCDEFi64, &1i64, &2i64, &0i64, &1i64],
            )
            .unwrap();
        }

        let characters = source.for_account(account);
        assert_eq!(characters.len(), 1, "{characters:?}");
        assert_eq!(characters[0].name, "Listed One");
        assert_eq!(characters[0].settings.appearance, 0xAB_CDEF);
        assert_eq!(characters[0].settings.level, 1);
        assert_eq!(
            characters[0].settings.to_blob().len(),
            crate::characters::CHARACTER_SETTINGS_LEN,
            "the list carries a whole settings block, whatever the character wears"
        );

        // And a different account's characters are not in it.
        assert!(source.for_account("00000000-0000-0000-0000-000000000000").is_empty());

        // The whole table lists it too - the administration tool's own view.
        assert!(
            source.list().iter().any(|character| character.name == "Listed One"),
            "the list holds the character"
        );

        // R14: the outpost the character is in is recorded, and the next read of the list says so.
        source.set_last_outpost(uuid, 148).expect("the outpost is recorded");
        let listed = source.list();
        assert_eq!(
            listed.iter().find(|character| character.name == "Listed One").map(|character| character.settings.last_outpost),
            Some(148),
            "the row now says the character is in map 148"
        );

        {
            let mut pool = pool.lock().unwrap();
            pool.query_with("DELETE FROM \"characters\" WHERE \"uuid\" = $1", &[&uuid])
                .unwrap();
        }
    }

    /// The live world database, when this machine has one: the item templates live there.
    fn world_source() -> Option<crate::data_stores::DatabaseItems> {
        let log = Log::new();
        let config =
            Config::load_json("../worldserver/worldserver.json", false, Policy::default()).ok()?;
        let settings = read_settings(&config, DatabaseKind::World).ok()?;
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::World, &settings, &log).ok()?;
        pool.ping().ok()?;
        Some(crate::data_stores::DatabaseItems::new(
            Arc::new(Mutex::new(pool)),
            log,
        ))
    }

    #[test]
    #[ignore = "needs the character database"]
    fn a_starting_item_write_answers_the_rows_it_wrote() {
        // The count is what the confirm's log line says, so it must be the rows, not the INSERT's
        // own answer: a statement without RETURNING answers no row even when it writes one, and a
        // log line that says "0 row(s) written" over written rows is a lie a load then trusts.
        //
        // Serialized with the other live tests that apply schemas to the same databases, so a run
        // of the whole suite cannot interleave a DDL pass with this write.
        let _serial = database::updater::apply::test_lock("characters");
        let _serial_world = database::updater::apply::test_lock("world");
        let Some(source) = source() else {
            panic!("the character database is not reachable");
        };
        let Some(items) = world_source() else {
            panic!("the world database is not reachable");
        };
        let log = Log::new();
        let mut world = crate::world::World::new(2);
        world
            .load_creation("../data/inventory.json", "../data/equipment.json", &log)
            .expect("the creation data");
        let tables = world.creation();
        let inventory = tables.inventory(0, 1);
        assert!(!inventory.items.is_empty(), "the set has items");

        // The templates the program reads back at start-up, from the same rows in the world database.
        items
            .seed(&tables.definitions())
            .expect("the definitions are seeded");
        let templates = items.list().expect("the templates are readable");

        let uuid = "00000000-0000-0000-0000-0000000000AB";
        let account = "00000000-0000-0000-0000-00000000C10D";
        {
            let mut pool = source.pool.lock().unwrap();
            for statement in [
                "DELETE FROM \"item_instance\" WHERE \"character_uuid\" = $1",
                "DELETE FROM \"characters\" WHERE \"uuid\" = $1",
            ] {
                pool.query_with(statement, &[&uuid]).unwrap();
            }
            pool.query_with(
                "INSERT INTO \"characters\" (\"uuid\", \"account_uuid\", \"name\", \"appearance\", \"primary_profession\", \"unlocked_professions\", \"campaign\", \"level\") \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[&uuid, &account, &"Written One", &0i64, &1i64, &2i64, &0i64, &1i64],
            )
            .unwrap();
        }

        let written = source
            .write_starting_items(uuid, &inventory, &templates)
            .expect("the starting items are written");
        let stored = source
            .items_of(uuid)
            .expect("the items are readable");
        // The count the confirm logs is the rows that are there.
        assert_eq!(
            written,
            stored.len(),
            "the count is the rows written"
        );
        // And every item of the set became a row: a missing template would leave one out, with a
        // warning of its own.
        assert_eq!(
            stored.len(),
            inventory.items.len(),
            "every item of the set is a row"
        );

        {
            let mut pool = source.pool.lock().unwrap();
            pool.query_with(
                "DELETE FROM \"item_instance\" WHERE \"character_uuid\" = $1",
                &[&uuid],
            )
            .unwrap();
            pool.query_with("DELETE FROM \"characters\" WHERE \"uuid\" = $1", &[&uuid])
                .unwrap();
        }
    }

    #[test]
    #[ignore = "needs the character database"]
    fn a_character_is_removed_with_its_own_records() {
        // The administration tool's remove: the row, its items and its unlocks, in one call. A row
        // that is not there answers false rather than failing, so a second click is harmless.
        let _serial = database::updater::apply::test_lock("characters");
        let Some(source) = source() else {
            panic!("the character database is not reachable");
        };
        let uuid = "00000000-0000-0000-0000-0000000000AC";
        let account = "00000000-0000-0000-0000-00000000C10D";
        {
            let mut pool = source.pool.lock().unwrap();
            pool.query_with(
                "INSERT INTO \"characters\" (\"uuid\", \"account_uuid\", \"name\", \"appearance\", \"primary_profession\", \"unlocked_professions\", \"campaign\", \"level\") \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[&uuid, &account, &"Removable One", &0i64, &1i64, &2i64, &0i64, &1i64],
            )
            .expect("the character row is written");
            pool.query_with(
                "INSERT INTO \"item_instance\" (\"character_uuid\", \"template_id\", \"file_id\", \"dye_tint\", \"dye_colors\", \"item_id\", \"bag_id\", \"slot\", \"quantity\", \"profession\") \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                &[&uuid, &1i64, &100i64, &0i64, &0i64, &9001i64, &1i64, &0i64, &1i64, &1i64],
            )
            .expect("the item row is written");
            pool.query_with(
                "INSERT INTO \"character_unlock\" (\"character_uuid\", \"map_id\") VALUES ($1, $2)",
                &[&uuid, &148i64],
            )
            .expect("the unlock row is written");
        }

        assert!(source.remove(uuid).expect("the character is removed"), "the row was there");
        assert!(
            !source.remove(uuid).expect("a second remove answers"),
            "nothing is left to remove"
        );
        assert!(
            source.items_of(uuid).expect("the items are readable").is_empty(),
            "its items are gone"
        );
        assert!(
            source
                .unlocked_maps(uuid)
                .expect("the unlocks are readable")
                .is_empty(),
            "its unlocks are gone"
        );
    }
}
