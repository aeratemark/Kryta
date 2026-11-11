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

const INSERT_HELD_ITEM: &str = "INSERT INTO \"item_instance\" (\"character_uuid\", \"template_id\", \
\"file_id\", \"dye_tint\", \"dye_colors\", \"item_id\", \"bag_id\", \"slot\", \"quantity\", \
\"profession\") VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)";

const SELECT_HELD_ITEMS: &str = "SELECT \"file_id\"::bigint, \"dye_tint\"::bigint, \
\"dye_colors\"::bigint, \"item_id\"::bigint, \"bag_id\"::bigint, \"slot\"::bigint, \
\"quantity\"::bigint, \"profession\"::bigint, \"template_id\"::bigint FROM \"item_instance\" \
WHERE \"character_uuid\" = $1 ORDER BY \"bag_id\", \"slot\"";

/// One item a character holds: which template it is, the item's own id, and where it sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldItem {
    /// The `item_template` row the item was written from; 0 for a row written before the column.
    pub template_id: i64,
    /// The `Gw.dat` file id of the definition.
    pub file_id: u32,
    /// Which dye slot mask applies.
    pub dye_tint: u8,
    /// The four packed dye colours.
    pub dye_colors: u16,
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
            let Some(template) = templates.iter().find(|template| {
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
                    && template.name == item.name
                    && template.modifiers == item.modifiers
            }) else {
                self.log.out(
                    LogLevel::Warn,
                    "server.characters",
                    format!(
                        "The creation set names item {} (file {}), which no item template describes; it is not written.",
                        item.id, item.file_id
                    ),
                );
                continue;
            };
            let reply = match self.pool.lock() {
                Ok(pool) => pool.query_reply(
                    INSERT_HELD_ITEM,
                    vec![
                        Param::Text(uuid.to_string()),
                        Param::Integer(template.id),
                        Param::Integer(i64::from(item.file_id)),
                        Param::Integer(i64::from(item.dye_tint)),
                        Param::Integer(i64::from(item.dye_colors)),
                        Param::Integer(i64::from(item.id)),
                        Param::Integer(i64::from(item.bag_id.unwrap_or(0))),
                        Param::Integer(i64::from(item.slot.unwrap_or(0))),
                        Param::Integer(i64::from(item.quantity)),
                        Param::Integer(i64::from(item.profession)),
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
                })
            })
            .collect()
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
}
