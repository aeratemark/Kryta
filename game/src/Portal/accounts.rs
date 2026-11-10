//! Who the portal says an account is, read from a configuration file.
//!
//! This is this project's own and it is temporary: a real identity store is a schema decision and
//! the login database has no account table. Until that decision, `data/accounts.json` holds the rows
//! a client can log in with:
//!
//! ```json
//! {
//!   "accounts": [
//!     {"user": "player@example.com", "user_id": "0F5A2C1E-...", "token": "AAAAAAAA-..."}
//!   ]
//! }
//! ```
//!
//! The login name, then the `user_id` and the `token` the client will carry. It is a record list and
//! not a set of settings keys, so it is read as rows through the one configuration reader rather than
//! flattened into `user = ...` lines. A file nobody set up is not an error: it means no account can
//! log in, and the portal says so with the reference's own answer rather than inventing an identity.

use std::collections::BTreeMap;

use common::configuration::json::Document;

use super::{Account, Accounts};

/// The accounts a client may log in as.
#[derive(Debug, Default, Clone)]
pub struct FileAccounts {
    entries: BTreeMap<String, Account>,
}

impl FileAccounts {
    /// Read the file. A missing file is an empty set, not a failure: it is the shape of a machine
    /// nobody has set up yet, and the portal's refusals already say what that means.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let document = Document::read(path).map_err(|error| error.to_string())?;
        let records = document.records("accounts")?;

        let mut entries = BTreeMap::new();
        for record in records {
            let login_name = record.required("user")?;
            let account = Account {
                user_id: record.required("user_id")?,
                token: record.required("token")?,
            };
            if entries.insert(login_name.clone(), account).is_some() {
                return Err(format!(
                    "{}: '{}' is stated twice",
                    path.display(),
                    login_name
                ));
            }
        }

        Ok(Self { entries })
    }

    /// The account behind a login name.
    pub fn get(&self, login_name: &str) -> Option<&Account> {
        self.entries.get(login_name)
    }

    /// How many accounts the file states.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the file states no account at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Accounts for FileAccounts {
    fn resolve(&self, login_name: &str) -> Option<Account> {
        self.entries.get(login_name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, text: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kryta-portal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("accounts.json");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn an_account_file_is_read_and_resolved() {
        let path = temp_file(
            "one",
            r#"{"accounts": [{"user": "player@example.com", "user_id": "USER-1", "token": "TOKEN-1"}]}"#,
        );
        let accounts = FileAccounts::load(&path).unwrap();

        assert_eq!(accounts.len(), 1);
        let account = accounts.resolve("player@example.com").unwrap();
        assert_eq!(account.user_id, "USER-1");
        assert_eq!(account.token, "TOKEN-1");
        assert!(accounts.resolve("nobody@example.com").is_none());
    }

    #[test]
    fn the_doc_beside_a_row_is_not_a_field() {
        // A shared file explains itself; the reader ignores what is not a field of a row.
        let path = temp_file(
            "doc",
            r#"{"accounts": [{"user": "a@example.com", "user_id": "U", "token": "T",
                              "doc": "the name this machine's client types"}]}"#,
        );
        let accounts = FileAccounts::load(&path).unwrap();
        assert_eq!(accounts.len(), 1);
    }

    #[test]
    fn a_missing_file_is_an_empty_set_rather_than_a_failure() {
        let dir = std::env::temp_dir().join(format!("kryta-portal-missing-{}", std::process::id()));
        let accounts = FileAccounts::load(dir.join("nothing.json")).unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn a_row_without_a_token_is_refused_with_its_row() {
        let path = temp_file(
            "bad",
            r#"{"accounts": [{"user": "player@example.com", "user_id": "USER-1"}]}"#,
        );
        let error = FileAccounts::load(&path).unwrap_err();
        assert!(error.contains("accounts[1]"), "{error}");
        assert!(error.contains("'token'"), "{error}");
    }

    #[test]
    fn a_name_stated_twice_is_refused() {
        let path = temp_file(
            "twice",
            r#"{"accounts": [
                 {"user": "a@example.com", "user_id": "U", "token": "T"},
                 {"user": "a@example.com", "user_id": "U2", "token": "T2"}
               ]}"#,
        );
        let error = FileAccounts::load(&path).unwrap_err();
        assert!(error.contains("stated twice"), "{error}");
    }

    #[test]
    fn the_shipped_accounts_hold_the_name_this_machines_client_types() {
        let accounts = FileAccounts::load("../data/accounts.json").unwrap();
        assert!(accounts.resolve("user1@example.com").is_some());
        assert_eq!(accounts.len(), 2);
    }
}
