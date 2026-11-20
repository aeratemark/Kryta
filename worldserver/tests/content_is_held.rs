//! Content is held, not re-read: a request path never touches the filesystem for it.
//!
//! The server is built to carry many players, their instances and their items. A request that read a
//! JSON file would put a file read, a parse and a lock on the hot path of every one of them, so the
//! world reads its content once at start-up (`Pack`, `Creation::Tables`, the command table), keeps it in
//! memory, and answers from memory for the life of the process (`docs/port/04-configuration.md`).
//!
//! This check fails, naming the file and the line, when a module that serves a connection reads a file.

use std::path::{Path, PathBuf};

/// The directories that run while a client is connected.
const SERVING: [&str; 4] = [
    "../game/src/Handlers",
    "../game/src/Server/Packets",
    "../game/src/Districts",
    "../game/src/Agents",
];

/// Reading a file, or parsing one.
const READS: [&str; 4] = [
    "Document::read",
    "read_to_string",
    "std::fs::read",
    "fs::read(",
];

#[test]
fn no_serving_module_reads_a_file() {
    let mut reads = Vec::new();
    for folder in SERVING {
        for file in sources(Path::new(folder)) {
            let text = std::fs::read_to_string(&file).expect("a source file reads");
            for (number, line) in text.lines().enumerate() {
                let statement = line.trim_start();
                if statement.starts_with("//") || statement.starts_with("///") {
                    continue;
                }
                for pattern in READS {
                    if statement.contains(pattern) {
                        reads.push(format!("{}:{}: {statement}", file.display(), number + 1));
                    }
                }
            }
        }
    }
    assert!(
        reads.is_empty(),
        "these modules serve connections and read a file; content is read once at start-up and held \
         in memory (docs/port/04-configuration.md):\n{}",
        reads.join("\n")
    );
}

/// Every `.rs` file under `root`.
fn sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(sources(&path));
        } else if path.extension().and_then(|kind| kind.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files
}
