//! An account: who may log in, and the state that belongs to the person rather than to a character.
//!
//! Reference: the reference's `src/server/game/Accounts/`, and the `account` table both programs read
//! (`docs/decisions/0006-accounts-are-rows.md`). This module is that noun's data access: the queries
//! live here, in the library, so the auth program, the world program and the administration tool ask
//! the same code for the same rows instead of each holding its own copy of the SQL.
//!
//! A row is state, never a setting: an account is created by an operator and changed by playing, which
//! is why the EULA version is a column (`data/sql/base/db_auth/account_eula.sql`) and not a key in a
//! file.

use std::sync::{Arc, Mutex};

use common::logging::{Log, LogLevel};
use database::database::{DatabaseWorkerPool, Param};

/// One account row, as the server and the tool read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// The row's own id.
    pub id: i64,
    /// The login name the client types.
    pub username: String,
    /// The portal identifier the client carries in its login message.
    pub user_id: String,
    /// The token the portal issued.
    pub token: String,
    /// The EULA version this account accepted, or 0 for none.
    pub eula: i64,
}

const SELECT_ALL: &str = "SELECT \"id\"::bigint, \"username\", \"user_id\", \"token\", \"eula\"::bigint FROM \"account\" ORDER BY \"id\"";

const SELECT_ONE: &str = "SELECT \"id\"::bigint, \"username\", \"user_id\", \"token\", \"eula\"::bigint FROM \"account\" WHERE \"username\" = $1";

/// An account is created with the identifiers the portal will issue for it, so a fresh account can log
/// in without a second step.
const INSERT_ACCOUNT: &str = "INSERT INTO \"account\" (\"username\", \"user_id\", \"token\") VALUES ($1, $2, $3) ON CONFLICT (\"username\") DO NOTHING RETURNING \"id\"::bigint";

// The version travels as text and is cast in the statement: a value from a form is text, and one cast
// here keeps the pool's parameter types out of the shape of the query.
const SET_EULA: &str = "UPDATE \"account\" SET \"eula\" = $2::text::int WHERE \"username\" = $1::text RETURNING \"id\"::bigint";

const DELETE_ACCOUNT: &str = "DELETE FROM \"account\" WHERE \"username\" = $1 RETURNING \"id\"::bigint";

/// The account table, through a worker pool.
pub struct DatabaseAccounts {
    pool: Arc<Mutex<DatabaseWorkerPool>>,
    log: Log,
}

impl DatabaseAccounts {
    /// A repository that asks this pool.
    pub fn new(pool: Arc<Mutex<DatabaseWorkerPool>>, log: Log) -> Self {
        Self { pool, log }
    }

    /// Every account, oldest first.
    pub fn list(&self) -> Result<Vec<Account>, String> {
        self.query(SELECT_ALL, Vec::new())?
            .iter()
            .map(account_of)
            .collect()
    }

    /// One account by the name the client types.
    pub fn find(&self, username: &str) -> Result<Option<Account>, String> {
        let rows = self.query(SELECT_ONE, vec![Param::Text(username.to_string())])?;
        rows.first().map(account_of).transpose()
    }

    /// Create an account, with the identifiers its portal login will present.
    ///
    /// Answers whether the row was written: a name that is taken writes nothing, and the caller says so
    /// rather than pretending.
    pub fn create(&self, username: &str, user_id: &str, token: &str) -> Result<bool, String> {
        let rows = self.query(
            INSERT_ACCOUNT,
            vec![
                Param::Text(username.to_string()),
                Param::Text(user_id.to_string()),
                Param::Text(token.to_string()),
            ],
        )?;
        Ok(!rows.is_empty())
    }

    /// Record the EULA version an account accepted, or 0 to make the client ask again.
    pub fn set_eula(&self, username: &str, version: i64) -> Result<bool, String> {
        let rows = self.query(
            SET_EULA,
            vec![Param::Text(username.to_string()), Param::Text(version.to_string())],
        )?;
        Ok(!rows.is_empty())
    }

    /// Record the EULA version for the account the portal identified, by its own identifier.
    ///
    /// The client accepts the EULA before the character list with only a version byte; the session
    /// knows the account by the portal's uuid, and this is the same column `set_eula` writes by name.
    pub fn set_eula_for_user(&self, user_id: &str, version: i64) -> Result<bool, String> {
        let rows = self.query(
            "UPDATE \"account\" SET \"eula\" = $2::text::int WHERE \"user_id\" = $1::text RETURNING \"id\"::bigint",
            vec![Param::Text(user_id.to_string()), Param::Text(version.to_string())],
        )?;
        Ok(!rows.is_empty())
    }

    /// Remove an account. Its characters are the character database's business and are left alone.
    pub fn delete(&self, username: &str) -> Result<bool, String> {
        let rows = self.query(DELETE_ACCOUNT, vec![Param::Text(username.to_string())])?;
        Ok(!rows.is_empty())
    }

    /// Run one statement and read every row back, or say why it could not.
    fn query(&self, sql: &str, params: Vec<Param>) -> Result<Vec<postgres::Row>, String> {
        let reply = match self.pool.lock() {
            Ok(pool) => pool.query_reply(sql, params),
            Err(_) => return Err("the login database pool is poisoned".to_string()),
        };
        match reply.recv() {
            Ok(Ok(rows)) => Ok(rows),
            Ok(Err(error)) => {
                self.log.out(
                    LogLevel::Error,
                    "server.authserver",
                    format!("the account table could not be read: {error}"),
                );
                Err(error.to_string())
            }
            Err(_) => Err("the login database answered no query: it is shutting down".to_string()),
        }
    }
}

/// One row, as an [`Account`]. A column of the wrong width is an error, not a panic: a page that cannot
/// read a row says so instead of taking the process down with it.
fn account_of(row: &postgres::Row) -> Result<Account, String> {
    let read = |column: &str| {
        row.try_get::<_, String>(column)
            .map_err(|error| format!("column {column}: {error}"))
    };
    let read_number = |column: &str| {
        row.try_get::<_, i64>(column)
            .map_err(|error| format!("column {column}: {error}"))
    };
    Ok(Account {
        id: read_number("id")?,
        username: read("username")?,
        user_id: read("user_id")?,
        token: read("token")?,
        eula: read_number("eula")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::configuration::{Config, Policy};
    use database::database::{read_settings, DatabaseKind};

    /// The live login database, when this machine has one.
    fn repository() -> Option<DatabaseAccounts> {
        let log = Log::new();
        let config =
            Config::load_json("../authserver/authserver.json", false, Policy::default()).ok()?;
        let settings = read_settings(&config, DatabaseKind::Login).ok()?;
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::Login, &settings, &log).ok()?;
        pool.ping().ok()?;
        Some(DatabaseAccounts::new(Arc::new(Mutex::new(pool)), log))
    }

    #[test]
    #[ignore = "needs the login database"]
    fn an_account_can_be_created_read_changed_and_removed() {
        let Some(accounts) = repository() else {
            panic!("the login database is not reachable");
        };
        let name = "manage-test@example.com";
        let _ = accounts.delete(name);

        assert!(
            accounts
                .create(
                    name,
                    "00000000-0000-0000-0000-00000000AAA1",
                    "00000000-0000-0000-0000-00000000AAA2"
                )
                .expect("created")
        );
        let row = accounts.find(name).expect("read").expect("the row is there");
        assert_eq!(row.username, name);
        assert_eq!(row.eula, 0, "a new account has accepted no EULA");

        assert!(accounts.set_eula(name, 26).expect("the EULA is set"));
        assert_eq!(accounts.find(name).expect("read").expect("there").eula, 26);

        assert!(accounts.delete(name).expect("removed"));
        assert!(accounts.find(name).expect("read").is_none());
    }
}
