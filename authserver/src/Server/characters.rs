//! Where a login's character list comes from: one query on the character database.
//!
//! The login's answer carries one `0x0007` per character, and each of those is a row of
//! `kryta_characters.characters` read back (`docs/decisions/0007-character-schema.md`). A character is
//! the player's, so it is a row and not a file - the same line the account table sits on.
//!
//! Reference: `AuthSrv_HandlePortalAccountLogin`, whose character list is a query of the character
//! database. The list is the account's, found by the portal's own identifier, which is the one the
//! login message carries.

use std::sync::{Arc, Mutex};

use common::logging::{Log, LogLevel};
use database::database::{DatabaseWorkerPool, Param};
use game::characters::{Character, CharacterSettings, uuid_bytes};

/// The characters of one account, in the order the list shows them.
const SELECT_CHARACTERS: &str = "SELECT \"uuid\", \"name\", \"appearance\", \"primary_profession\", \
\"secondary_profession\", \"campaign\", \"level\", \"is_pvp\", \"helm_status\", \"last_outpost\" \
FROM \"characters\" WHERE \"account_uuid\" = $1 ORDER BY \"name\"";

/// The character database, as the login program uses it.
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
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(SELECT_CHARACTERS, vec![Param::Text(account_uuid.to_string())]),
            Err(_) => {
                self.log.out(
                    LogLevel::Error,
                    "server.authserver",
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
                    "server.authserver",
                    format!("Cannot read the characters of {account_uuid}: {error}"),
                );
                Vec::new()
            }
            Err(_) => {
                self.log.out(
                    LogLevel::Error,
                    "server.authserver",
                    "the character database answered no query: it is shutting down",
                );
                Vec::new()
            }
        }
    }

    /// One row, as the list reads it.
    fn character(&self, row: &postgres::Row) -> Option<Character> {
        let uuid_text: String = row.get("uuid");
        let uuid = match uuid_bytes(&uuid_text) {
            Ok(uuid) => uuid,
            Err(error) => {
                self.log.out(
                    LogLevel::Error,
                    "server.authserver",
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
            game::characters::CHARACTER_SETTINGS_LEN,
            "the list carries a whole settings block, whatever the character wears"
        );

        // And a different account's characters are not in it.
        assert!(source.for_account("00000000-0000-0000-0000-000000000000").is_empty());
    }
}
