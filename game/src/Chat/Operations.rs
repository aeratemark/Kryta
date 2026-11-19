//! Registered operations shared by operator surfaces.
//!
//! Reference: `src/server/game/Chat/ChatCommands/ChatCommand.h` - a command is a name, a handler, a
//! permission and whether a surface may run it - and `ChatCommand.cpp:513-523`, which decides what a
//! surface may ask for. Execution order is `src/server/game/World/World.cpp:1721-1736` at 30a36ec4d:
//! the request is queued and the world update runs it.
//!
//! **One operation is one registered function, called with what the caller supplies.** A caller does
//! not reach a function through a dispatcher arm: it names the operation, supplies the arguments the
//! registration declares and the caller it is, and receives the operation's answer or a refusal with a
//! code. Adding an operation is a registration; adding a command that runs an existing operation is a
//! row in `data/commands.json`; neither edits a door (`docs/port/14-operator-surfaces.md`).

use std::collections::BTreeMap;

/// Who is asking. The adapter supplies this; a caller never names itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    /// The kind of surface the request arrived through.
    pub kind: CallerKind,
    /// The identity that surface vouched for; empty when it has none, as the local console has none.
    pub name: String,
}

/// The surfaces a request may arrive through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallerKind {
    /// The embedded console: standard input of the world program.
    Console,
    /// The local control endpoint, authenticated by the service credential.
    Api,
    /// An in-game chat line, once a player command calls an operation.
    Chat,
    /// A script, once the Lua runtime exists.
    Lua,
}

impl Caller {
    /// The local console, which is the machine the world runs on.
    pub fn console() -> Self {
        Self { kind: CallerKind::Console, name: String::new() }
    }

    /// A caller vouched for by the local control endpoint.
    pub fn api(name: &str) -> Self {
        Self { kind: CallerKind::Api, name: name.to_string() }
    }

    /// The kind's name, for a result that says who asked.
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            CallerKind::Console => "console",
            CallerKind::Api => "api",
            CallerKind::Chat => "chat",
            CallerKind::Lua => "lua",
        }
    }
}

/// The values an operation was called with, in the order its registration declares them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Arguments {
    values: Vec<String>,
}

impl Arguments {
    /// No values: what an operation that declares none is called with.
    pub fn none() -> Self {
        Self::default()
    }

    /// Values in declaration order.
    pub fn of(values: Vec<String>) -> Self {
        Self { values }
    }

    /// How many values were supplied.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether none were supplied.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// One value by position.
    pub fn get(&self, index: usize) -> Option<&str> {
        self.values.get(index).map(String::as_str)
    }

    /// Every value, in declaration order.
    pub fn values(&self) -> &[String] {
        &self.values
    }
}

/// Why an operation refused. The code is what a machine reads; the message is what a person reads.
///
/// Reference: the reference renders refusals as text through the handler (`SendErrorMessage`,
/// `Chat.cpp:167`), so the text is the surface's, and the code is this tree's way of telling a machine
/// which refusal it was without parsing that text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// A stable name for the refusal: what a caller branches on.
    pub code: &'static str,
    /// What a person reads. A surface may render it verbatim, and the reference's wording is kept
    /// where the reference has one.
    pub message: String,
}

/// The refusal for a name no surface grants.
pub const UNKNOWN_OPERATION: &str = "unknown_operation";
/// The refusal for a call whose arguments do not match its registration.
pub const INVALID_ARGUMENTS: &str = "invalid_arguments";
/// A refusal the operation itself decides on.
pub const REFUSED: &str = "refused";

impl Refusal {
    /// One refusal, by code and message.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

/// One call: what is being asked, with what, by whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The registered operation's name.
    pub operation: String,
    /// The values it was called with, in its own declaration order.
    pub arguments: Arguments,
    /// Who is asking.
    pub caller: Caller,
}

/// What an operation is: the facts a door needs before it calls the function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registration {
    /// The stable name a row or an endpoint names.
    pub name: &'static str,
    /// What the operation does, in one line, for help and for a manager page.
    pub help: &'static str,
    /// The values the operation takes, in order. Empty means it takes none.
    pub arguments: &'static [&'static str],
}

/// What an operation does: one function of the call and the world it acts on.
pub type Handler = fn(&Invocation, &crate::world::World) -> Result<String, Refusal>;

struct Entry {
    registration: Registration,
    handler: Handler,
}

/// The operations a world has registered.
///
/// A registry, not a dispatcher: a name that is not registered is refused with `unknown_operation`,
/// and there is no branch a new operation could be spliced into.
#[derive(Default)]
pub struct Registry {
    entries: BTreeMap<&'static str, Entry>,
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one implementation, refusing a duplicate or empty name.
    pub fn register(&mut self, registration: Registration, handler: Handler) -> Result<(), String> {
        if registration.name.trim().is_empty() {
            return Err("an operation needs a name".into());
        }
        if self.entries.contains_key(registration.name) {
            return Err(format!("operation {:?} is registered twice", registration.name));
        }
        self.entries.insert(registration.name, Entry { registration, handler });
        Ok(())
    }

    /// What an operation is, if it is registered.
    pub fn get(&self, name: &str) -> Option<Registration> {
        self.entries.get(name).map(|entry| entry.registration)
    }

    /// Whether an operation is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// Every registration, by name.
    pub fn rows(&self) -> impl Iterator<Item = Registration> + '_ {
        self.entries.values().map(|entry| entry.registration)
    }

    /// Build a call, checking that the operation exists and that the values match its declaration.
    ///
    /// This is where "unknown operation" and "invalid arguments" are decided, before any surface's
    /// rendering and before the world is touched.
    pub fn invocation(
        &self,
        name: &str,
        arguments: Arguments,
        caller: Caller,
    ) -> Result<Invocation, Refusal> {
        let Some(registration) = self.get(name) else {
            return Err(Refusal::new(UNKNOWN_OPERATION, format!("unknown operation: {name}")));
        };
        if arguments.len() != registration.arguments.len() {
            return Err(Refusal::new(
                INVALID_ARGUMENTS,
                format!(
                    "operation {name} takes {} argument(s) ({}), and {} were supplied",
                    registration.arguments.len(),
                    if registration.arguments.is_empty() {
                        "none".to_string()
                    } else {
                        registration.arguments.join(", ")
                    },
                    arguments.len()
                ),
            ));
        }
        Ok(Invocation { operation: name.to_string(), arguments, caller })
    }

    /// Run a call against a world. Called on the world's own thread, never on a listener's.
    pub fn invoke(&self, invocation: &Invocation, world: &crate::world::World) -> Result<String, Refusal> {
        let Some(entry) = self.entries.get(invocation.operation.as_str()) else {
            return Err(Refusal::new(
                UNKNOWN_OPERATION,
                format!("unknown operation: {}", invocation.operation),
            ));
        };
        (entry.handler)(invocation, world)
    }
}

/// Existing world information, presented to an operator.
pub fn world_status(_invocation: &Invocation, world: &crate::world::World) -> Result<String, Refusal> {
    Ok(format!(
        "World {}: {} map(s) loaded.",
        world.realm(),
        world.maps().len()
    ))
}

/// Build both outputs of the existing map-list operation.
///
/// ASURA: `crates/asura-world/src/chat.rs:41-102,288-324`. The chat surface sends the chat half and
/// logs the table; the console prints the table (`map_targets` is called by both, so the text is one
/// implementation).
pub fn map_targets(maps: &[crate::world::MapEntry]) -> crate::chat::MappedMaps {
    crate::chat::maps_answer(maps)
}

/// The map list, presented to an operator: the table the chat surface only logs.
pub fn list_targets(_invocation: &Invocation, world: &crate::world::World) -> Result<String, Refusal> {
    Ok(map_targets(world.maps()).table.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        let mut registry = Registry::new();
        registry
            .register(
                Registration {
                    name: "world.status",
                    help: "what this world is running",
                    arguments: &[],
                },
                world_status,
            )
            .unwrap();
        registry
    }

    #[test]
    fn registration_refuses_a_duplicate_or_empty_name() {
        let mut registry = Registry::new();
        let registration = Registration { name: "world.status", help: "", arguments: &[] };
        registry.register(registration, world_status).unwrap();
        assert!(registry.register(registration, world_status).is_err(), "a name is registered once");
        let unnamed = Registration { name: "  ", help: "", arguments: &[] };
        assert!(Registry::new().register(unnamed, world_status).is_err(), "a name is required");
    }

    #[test]
    fn an_unregistered_name_is_refused_by_code_not_by_parsing_text() {
        let registry = registry();
        let refusal = registry
            .invocation("nope", Arguments::none(), Caller::console())
            .unwrap_err();
        assert_eq!(refusal.code, UNKNOWN_OPERATION);
        assert_eq!(refusal.message, "unknown operation: nope");
    }

    #[test]
    fn the_argument_count_is_checked_against_the_registration() {
        let registry = registry();
        let refusal = registry
            .invocation("world.status", Arguments::of(vec!["/".into()]), Caller::console())
            .unwrap_err();
        assert_eq!(refusal.code, INVALID_ARGUMENTS);
        assert!(refusal.message.contains("takes 0 argument(s)"), "{}", refusal.message);
        assert!(registry
            .invocation("world.status", Arguments::none(), Caller::console())
            .is_ok());
    }

    #[test]
    fn the_caller_reaches_the_operation_and_the_world_answers() {
        fn who(invocation: &Invocation, _world: &crate::world::World) -> Result<String, Refusal> {
            Ok(format!("{}:{}", invocation.caller.kind_label(), invocation.caller.name))
        }
        let mut registry = Registry::new();
        registry
            .register(Registration { name: "who", help: "", arguments: &[] }, who)
            .unwrap();
        let world = crate::world::World::new(7);
        let invocation = registry.invocation("who", Arguments::none(), Caller::api("manage")).unwrap();
        assert_eq!(registry.invoke(&invocation, &world).unwrap(), "api:manage");
        let console = registry.invocation("who", Arguments::none(), Caller::console()).unwrap();
        assert_eq!(registry.invoke(&console, &world).unwrap(), "console:");
    }

    #[test]
    fn the_status_operation_reads_the_world_it_is_handed() {
        let registry = registry();
        let world = crate::world::World::new(7);
        let invocation = registry.invocation("world.status", Arguments::none(), Caller::console()).unwrap();
        assert_eq!(registry.invoke(&invocation, &world).unwrap(), "World 7: 0 map(s) loaded.");
    }
}
