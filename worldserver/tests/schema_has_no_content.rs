//! The boundary, as a check: `data/sql` is schema, and content is the pack.
//!
//! `docs/port/04-configuration.md` states it and `docs/decisions/0014-the-world-content-is-a-pack.md`
//! records it: a map record, its entry points and its doorways are the world pack's files, read at
//! start-up; a database row is for what a server *writes*. So a schema file may create, alter or drop
//! a table, and may never seed one - if a fresh server needs rows fed to it before it serves, the
//! content is in the wrong place.
//!
//! This is the rule with teeth. It fails by naming the file and the statement, which is what an
//! unenforced rule never does.

use std::path::{Path, PathBuf};

/// Every statement that writes rows, as opposed to shaping the schema.
const SEEDING: [&str; 3] = ["INSERT INTO", "UPDATE ", "DELETE FROM"];

#[test]
fn no_schema_file_seeds_content() {
    let mut seeding = Vec::new();
    for file in sql_files(Path::new("../data/sql")) {
        let text = std::fs::read_to_string(&file).expect("a schema file reads");
        for (number, line) in text.lines().enumerate() {
            let statement = line.trim_start();
            for pattern in SEEDING {
                // A comment that names the statement is documentation, not a seed.
                if statement.starts_with("--") {
                    break;
                }
                if statement.starts_with(pattern) {
                    seeding.push(format!("{}:{}: {statement}", file.display(), number + 1));
                }
            }
        }
    }
    assert!(
        seeding.is_empty(),
        "these schema files seed rows, which is content in SQL - it belongs in the world pack \
         (docs/decisions/0014-the-world-content-is-a-pack.md):\n{}",
        seeding.join("\n")
    );
}

/// Every `.sql` file under `root`, recursing.
fn sql_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(sql_files(&path));
        } else if path.extension().and_then(|kind| kind.to_str()) == Some("sql") {
            files.push(path);
        }
    }
    files
}
