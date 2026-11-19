//! The chat command table: which names a player may type, and the operation each one runs.
//!
//! Reference: `docs/decisions/0011-the-command-surface-is-content.md`. The command surface is content,
//! so the rows live in a file (`data/commands.json`) and Rust keeps only the operations. The reference's
//! own dispatch is two functions - `GameSrv_TryHandleGmCommand` (`GmCommands.c`, `/tp`, `/teleport`,
//! `/maps`) and `GameSrv_HandleChatCommand` (`GmChat.c`, `/stuck`) - and the working implementation
//! names that surface once in a Rust table (`ASURA/crates/asura-world/src/chat.rs:126-163`). This is
//! that table, read from a file instead.
//!
//! **A row binds a name to an operation, and the binding is checked when the file is read.** An
//! `execution` this server has no operation for is a refused row with a reason, so a typo in the data is
//! reported at start-up rather than answering "Unknown command" to a player who typed a name the file
//! promised. That is the same rule the profile loading follows: a missing required reference is a
//! validation error, never a silent no-op.

use common::configuration::json::Document;
use common::logging::{Log, LogLevel};

/// A server operation a command may run. The names in the file are these names, exactly.
///
/// This is the boundary `docs/port/11-content-and-behaviour.md` describes: the file says *which*
/// operation a name runs, and the operation itself - the transfer cycle, the alias table, the position
/// broadcast - stays in Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// A server transfer: `/tp`. `answer_teleport_command` carries it out.
    Teleport,
    /// The alias table: `/maps`.
    ListTargets,
    /// Where this agent stands: `/stuck`.
    Position,
}

impl Operation {
    /// Every name a file may write, in the order the refusals list them.
    pub const NAMES: &'static [&'static str] = &["Teleport", "ListTargets", "Position"];

    /// The operation a row names, or nothing when this server has no such operation.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Teleport" => Some(Operation::Teleport),
            "ListTargets" => Some(Operation::ListTargets),
            "Position" => Some(Operation::Position),
            _ => None,
        }
    }

    /// The name a file writes for this operation.
    pub fn name(self) -> &'static str {
        match self {
            Operation::Teleport => "Teleport",
            Operation::ListTargets => "ListTargets",
            Operation::Position => "Position",
        }
    }
}

/// One command row: the names it answers to, the operation it runs, and what it takes.
///
/// A row carries one of two bindings, because the two surfaces bind differently and always did:
/// `execution` names one of the closed chat operations below, and `operation` names a registration in
/// [`crate::chat::operations`](super::operations). A row may state `surfaces`; a row without it serves
/// the surface its binding is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// What a player types, without the slash (`GmCommands.c` and `GmChat.c` take lowercase names).
    pub command: String,
    /// The other names the same command answers to: `GmCommands.c` accepts two names for one command.
    pub also: Vec<String>,
    /// The chat operation it runs, or the registered operation it names.
    pub execution: Execution,
    /// The values it takes, in order. Empty means it takes none.
    pub parameters: Vec<String>,
    /// The surfaces that may run it.
    pub surfaces: Vec<Surface>,
}

/// What a row binds its name to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Execution {
    /// One of the chat operations: the reference's two dispatch points, as a closed set.
    Chat(Operation),
    /// A registered operation, by its stable name (`data/commands.json` says which).
    Registered(String),
}

/// A surface a command may be typed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// The in-game chat line.
    Chat,
    /// The embedded console, the local control endpoint and the standalone CLI.
    Console,
}

impl Surface {
    /// The name a row writes for this surface.
    pub fn name(self) -> &'static str {
        match self {
            Surface::Chat => "chat",
            Surface::Console => "console",
        }
    }

    /// The surface a row's name writes, if it writes one this tree has.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "chat" => Some(Surface::Chat),
            "console" => Some(Surface::Console),
            _ => None,
        }
    }
}

impl Command {
    /// Every name this command answers to, its own first.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.command.as_str()).chain(self.also.iter().map(String::as_str))
    }

    /// Whether it takes an argument.
    pub fn takes_argument(&self) -> bool {
        !self.parameters.is_empty()
    }

    /// Whether this row may be run at a surface.
    pub fn serves(&self, surface: Surface) -> bool {
        self.surfaces.contains(&surface)
    }

    /// The chat operation this row runs, if it runs one.
    pub fn chat_operation(&self) -> Option<Operation> {
        match &self.execution {
            Execution::Chat(operation) => Some(*operation),
            Execution::Registered(_) => None,
        }
    }

    /// The registered operation this row names, if it names one.
    pub fn registered(&self) -> Option<&str> {
        match &self.execution {
            Execution::Chat(_) => None,
            Execution::Registered(name) => Some(name),
        }
    }
}

/// Every command a world answers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    rows: Vec<Command>,
}

impl Table {
    /// An empty table: the state of a world started without a command file, which answers no command
    /// and says so once, at start-up.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Read the rows.
    ///
    /// A file that is missing is reported and leaves the table empty, the way the creation tables are
    /// handled: the program starts and the log says what it does not have. A file that is *present* but
    /// wrong is an error - a row without a name, a name stated twice, a parameter list the parser cannot
    /// honour, or an operation this server does not have - because a command that cannot work is worse
    /// than one that does not exist.
    pub fn load(path: impl AsRef<std::path::Path>, log: &Log) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            log.out(
                LogLevel::Warn,
                "server.worldserver",
                format!(
                    "No chat command table at {}: no chat command will be answered.",
                    path.display()
                ),
            );
            return Ok(Table::empty());
        }
        let document = Document::read(path).map_err(|error| error.to_string())?;
        let mut table = Table::empty();
        for record in document.records("commands")? {
            let command = record.required("command")?;
            let also = record.strings("also")?.unwrap_or_default();
            let execution = record.text("execution")?;
            let registered = record.text("operation")?;
            let parameters = record.strings("parameters")?.unwrap_or_default();
            let surfaces = record.strings("surfaces")?.unwrap_or_default();

            if command.is_empty() || command.contains('/') {
                return Err(format!(
                    "{}: a command is a name without the slash, and this row's is {command:?}",
                    path.display()
                ));
            }
            if command != command.to_lowercase() {
                return Err(format!(
                    "{}: command {command:?} is not lower case; the client sends a command as typed and the reference's names are lower case",
                    path.display()
                ));
            }
            // One binding, and the one its surface parses: a chat row runs a closed operation, a
            // console row names a registration the console asks the registry for.
            let (execution, default_surface) = match (execution, registered) {
                (Some(_), Some(_)) => {
                    return Err(format!(
                        "{}: command {command:?} states both `execution` and `operation`; a row binds one or the other",
                        path.display()
                    ))
                }
                (Some(name), None) => {
                    let Some(operation) = Operation::from_name(&name) else {
                        return Err(format!(
                            "{}: command {command:?} runs {name:?}, which is not an operation this server has; the operations are {}",
                            path.display(),
                            Operation::NAMES.join(", ")
                        ));
                    };
                    (Execution::Chat(operation), Surface::Chat)
                }
                (None, Some(name)) => (Execution::Registered(name), Surface::Console),
                (None, None) => {
                    return Err(format!(
                        "{}: command {command:?} states neither `execution` nor `operation`, so it runs nothing",
                        path.display()
                    ))
                }
            };
            let surfaces = if surfaces.is_empty() {
                vec![default_surface]
            } else {
                let mut resolved = Vec::with_capacity(surfaces.len());
                for name in &surfaces {
                    let Some(surface) = Surface::from_name(name) else {
                        return Err(format!(
                            "{}: command {command:?} names surface {name:?}; the surfaces are chat, console",
                            path.display()
                        ));
                    };
                    resolved.push(surface);
                }
                resolved
            };
            if matches!(execution, Execution::Chat(_)) && command.split_whitespace().count() != 1 {
                return Err(format!(
                    "{}: a chat command is one word, and this row's is {command:?}",
                    path.display()
                ));
            }
            if matches!(execution, Execution::Chat(_)) && parameters.len() > 1 {
                return Err(format!(
                    "{}: command {command:?} states {} parameters, and a chat line is split on its first space, so only one can be read",
                    path.display(),
                    parameters.len()
                ));
            }
            table.rows.push(Command {
                command,
                also,
                execution,
                parameters,
                surfaces,
            });
        }

        // One name, one row: two rows answering `tp` would answer it twice, and which one ran would be
        // the file's order rather than its meaning.
        for (index, row) in table.rows.iter().enumerate() {
            for name in row.names() {
                if let Some(other) = table
                    .rows
                    .iter()
                    .skip(index + 1)
                    .find(|candidate| candidate.names().any(|other| other == name))
                {
                    return Err(format!(
                        "{}: {name:?} is stated by two rows ({:?} and {:?})",
                        path.display(),
                        row.command,
                        other.command
                    ));
                }
            }
        }

        Ok(table)
    }

    /// The row a player's name resolves to, if this world answers it.
    pub fn command(&self, name: &str) -> Option<&Command> {
        self.rows
            .iter()
            .find(|row| row.names().any(|candidate| candidate == name))
    }

    /// The rows, in the file's order.
    pub fn rows(&self) -> &[Command] {
        &self.rows
    }

    /// The console row a typed line resolves to, and the values it carries.
    ///
    /// A console row's name may be more than one word (`server info` is the reference's name for a
    /// console command), so the match is the longest name that prefixes the line. The values are what
    /// remains, split on whitespace; a name that matches with the wrong number of values resolves to
    /// nothing, which is the reference's "Unknown command" rather than a partly-run command.
    pub fn resolve_console(&self, line: &str) -> Option<(&Command, Vec<String>)> {
        let mut best: Option<(&Command, Vec<String>)> = None;
        for row in self.rows.iter().filter(|row| row.serves(Surface::Console)) {
            for name in row.names() {
                let rest = if line == name {
                    ""
                } else if let Some(rest) = line.strip_prefix(name).and_then(|rest| rest.strip_prefix(' ')) {
                    rest.trim()
                } else {
                    continue;
                };
                let values: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
                if values.len() != row.parameters.len() {
                    continue;
                }
                if best.as_ref().is_none_or(|(found, _)| found.command.len() < name.len()) {
                    best = Some((row, values));
                }
            }
        }
        // One row per name is enforced at load, so a match here is unambiguous.
        best
    }

    /// How many commands this world answers.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether it answers none.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::configuration::json::Document;

    /// The file this tree ships, read as a world reads it.
    #[test]
    fn the_shipped_table_binds_the_references_chat_commands_and_the_consoles_status() {
        let document = Document::read("../data/commands.json").expect("the shipped file parses");
        let records = document.records("commands").expect("commands is a record list");
        assert_eq!(records.len(), 6);

        let table = Table::load("../data/commands.json", &Log::new()).expect("the shipped table reads");
        assert_eq!(table.len(), 6);
        let tp = table.command("tp").expect("/tp is answered");
        assert_eq!(tp.chat_operation(), Some(Operation::Teleport));
        assert!(tp.takes_argument());
        assert!(tp.serves(Surface::Chat));
        assert!(!tp.serves(Surface::Console), "a chat row is not a console row");
        assert_eq!(
            table.command("teleport").and_then(Command::chat_operation),
            Some(Operation::Teleport),
            "the reference accepts two names for one command"
        );
        assert_eq!(
            table.command("maps").and_then(Command::chat_operation),
            Some(Operation::ListTargets)
        );
        assert_eq!(
            table.command("stuck").and_then(Command::chat_operation),
            Some(Operation::Position)
        );
        assert!(!table.command("maps").unwrap().takes_argument());
        assert!(table.command("say").is_none());

        // The console row: the reference's own console name, bound to a registered operation.
        let info = table.command("server info").expect("the console answers `server info`");
        assert_eq!(info.registered(), Some("world.status"));
        assert!(!info.takes_argument());
        assert!(info.serves(Surface::Console));
        assert!(!info.serves(Surface::Chat), "a console row is not a chat row");
        let (row, values) = table.resolve_console("server info").expect("the console resolves it");
        assert_eq!(row.command, "server info");
        assert!(values.is_empty());
        assert!(table.resolve_console("server info now").is_none(), "it takes no values");
        assert!(table.resolve_console("tp ascalon").is_none(), "a chat row is not console-visible");

        // The two lifecycle rows the reference's console has (`cs_server.cpp:68,89`).
        for (name, operation) in [
            ("server exit", "world.exit"),
            ("server restart", "world.restart"),
        ] {
            let row = table.command(name).expect("the console answers it");
            assert_eq!(row.registered(), Some(operation));
            assert!(!row.takes_argument());
            assert!(row.serves(Surface::Console));
        }
    }

    /// A console row may be more than one word, and the longest name wins.
    #[test]
    fn the_console_resolves_the_longest_name_that_prefixes_the_line() {
        let text = r#"{"commands":[
            {"command":"server info","operation":"world.status","surfaces":["console"]},
            {"command":"server","operation":"world.status","surfaces":["console"]}
        ]}"#;
        let path = scratch("longest-name");
        std::fs::write(&path, text).unwrap();
        let table = Table::load(&path, &Log::new()).unwrap();
        let (row, _) = table.resolve_console("server info").unwrap();
        assert_eq!(row.command, "server info");
        let (row, _) = table.resolve_console("server").unwrap();
        assert_eq!(row.command, "server");
    }

    /// A row that binds nothing, or binds both ways, or names a surface this tree lacks, is refused.
    #[test]
    fn a_row_must_bind_one_surface_and_one_operation() {
        let cases = [
            r#"{"commands":[{"command":"x"}]}"#,
            r#"{"commands":[{"command":"x","execution":"Teleport","operation":"world.status"}]}"#,
            r#"{"commands":[{"command":"x","execution":"Teleport","surfaces":["telnet"]}]}"#,
        ];
        for (index, text) in cases.iter().enumerate() {
            let path = scratch(&format!("binding-{index}"));
            std::fs::write(&path, text).unwrap();
            assert!(Table::load(&path, &Log::new()).is_err(), "{text}");
        }
    }

    /// An operation the server does not have is refused when the file is read, not when a player types
    /// the name.
    #[test]
    fn a_row_naming_an_operation_this_server_lacks_is_refused() {
        let text = r#"{"commands":[{"command":"tp","execution":"Teleport"},{"command":"warp","execution":"Warp"}]}"#;
        let path = scratch("unknown-operation");
        std::fs::write(&path, text).unwrap();
        let error = Table::load(&path, &Log::new()).unwrap_err();
        assert!(error.contains("Warp"), "{error}");
        assert!(error.contains("Teleport"), "the refusal lists what exists: {error}");
    }

    /// Two rows answering one name would answer it twice.
    #[test]
    fn a_name_stated_twice_is_refused() {
        let text = r#"{"commands":[{"command":"tp","execution":"Teleport"},{"command":"go","also":["tp"],"execution":"Position"}]}"#;
        let path = scratch("duplicate-name");
        std::fs::write(&path, text).unwrap();
        let error = Table::load(&path, &Log::new()).unwrap_err();
        assert!(error.contains("stated by two rows"), "{error}");
    }

    /// The parser splits a line on its first space, so more than one parameter cannot be read.
    #[test]
    fn more_than_one_parameter_is_refused() {
        let text = r#"{"commands":[{"command":"warp","execution":"Teleport","parameters":["map","x"]}]}"#;
        let path = scratch("two-parameters");
        std::fs::write(&path, text).unwrap();
        let error = Table::load(&path, &Log::new()).unwrap_err();
        assert!(error.contains("first space"), "{error}");
    }

    /// A missing file is not an error: the world starts, answers no command, and says so.
    #[test]
    fn a_missing_file_leaves_the_table_empty() {
        let table = Table::load("../data/no-such-command-file.json", &Log::new()).unwrap();
        assert!(table.is_empty());
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("kryta-commands-{name}.json"));
        let _ = std::fs::remove_file(&path);
        path
    }
}
