//! Console input queues commands for the world update.
//! Reference: `CliRunnable.cpp:110-231` and `World.cpp:1721-1736` at 30a36ec4d.

use std::io::{BufRead, IsTerminal, Write};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};

pub struct Request {
    pub line: String,
    pub reply: Sender<String>,
}

/// Run pending commands from the world update thread.
pub fn process(
    receiver: &Receiver<Request>,
    registry: &game::chat::operations::Registry,
    world: &game::world::World,
) {
    while let Ok(request) = receiver.try_recv() {
        let answer = execute(registry, world, &request.line);
        let _ = request.reply.send(answer);
    }
}

/// Start only for an interactive terminal, leaving supervised input to its owner.
pub fn start(sender: SyncSender<Request>) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    std::thread::Builder::new()
        .name("world-console".into())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut input = stdin.lock();
            loop {
                print!("Kryta> ");
                let _ = std::io::stdout().flush();
                let mut line = String::new();
                if !matches!(input.read_line(&mut line), Ok(n) if n > 0) {
                    break;
                }
                let (reply, result) = mpsc::channel();
                if sender.send(Request { line, reply }).is_err() {
                    break;
                }
                match result.recv() {
                    Ok(answer) => println!("{answer}"),
                    Err(_) => break,
                }
            }
        })
        .is_ok()
}

/// Execute the available read-only console commands on the world thread.
pub fn execute(
    registry: &game::chat::operations::Registry,
    world: &game::world::World,
    line: &str,
) -> String {
    let line = line.trim();
    if line.is_empty() {
        return String::new();
    }
    if let Some(operation) = line.strip_prefix('@') {
        return registry.invoke(operation, world).unwrap_or_else(|error| error);
    }
    if line == "help" {
        let mut available = vec!["help".to_string(), "server info".to_string()];
        for command in world.commands().rows() {
            if !command.takes_argument() && registry.contains(command.operation.name()) {
                available.extend(command.names().map(str::to_string));
            }
        }
        return format!("Available commands: {}", available.join(", "));
    }
    let name = if line == "server info" {
        "world.status"
    } else {
        let (command_name, argument) = line.split_once(' ').unwrap_or((line, ""));
        let Some(command) = world.commands().command(command_name) else {
            return "Unknown command".into();
        };
        if command.takes_argument() || !registry.contains(command.operation.name()) {
            return "This command is not available from the console; it requires a player session."
                .into();
        }
        if !argument.trim().is_empty() {
            return "This command does not take arguments.".into();
        }
        command.operation.name()
    };
    registry.invoke(name, world).unwrap_or_else(|error| error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_reports_the_world_and_refuses_unknown_commands() {
        let world = game::world::World::new(7);
        let mut registry = game::chat::operations::Registry::new();
        registry
            .register("world.status", game::chat::operations::world_status)
            .unwrap();
        assert_eq!(
            execute(&registry, &world, "server info"),
            "World 7: 0 map(s) loaded."
        );
        assert_eq!(execute(&registry, &world, "unknown"), "Unknown command");
        assert!(execute(&registry, &world, "help").contains("server info"));
    }

    #[test]
    fn queued_command_is_executed_and_answers_the_caller() {
        let world = game::world::World::new(7);
        let mut registry = game::chat::operations::Registry::new();
        registry
            .register("world.status", game::chat::operations::world_status)
            .unwrap();
        let (sender, receiver) = mpsc::sync_channel(1);
        let (reply, result) = mpsc::channel();
        sender
            .send(Request {
                line: "server info".into(),
                reply,
            })
            .unwrap();
        assert!(result.try_recv().is_err());
        process(&receiver, &registry, &world);
        assert_eq!(result.recv().unwrap(), "World 7: 0 map(s) loaded.");
    }

    #[test]
    fn help_only_lists_commands_available_without_a_session() {
        let mut world = game::world::World::new(7);
        let table = game::chat::commands::Table::load(
            "../data/commands.json",
            &common::logging::Log::new(),
        )
        .unwrap();
        world.set_commands(table);
        let mut registry = game::chat::operations::Registry::new();
        registry
            .register("ListTargets", game::chat::operations::list_targets)
            .unwrap();
        let help = execute(&registry, &world, "help");
        assert!(help.contains("maps"));
        assert!(!help.contains("stuck"));
        assert_eq!(
            execute(&registry, &world, "maps extra"),
            "This command does not take arguments."
        );
        assert!(execute(&registry, &world, "tp ascalon").contains("requires a player session"));
    }
}
