//! Registered operations shared by operator surfaces.
//!
//! Command registration follows `src/server/game/Chat/ChatCommands/ChatCommand.cpp` and world
//! execution follows `src/server/game/World/World.cpp:1721-1736` at 30a36ec4d.

use std::collections::BTreeMap;

/// An operation which reads the world without a player session.
pub type Handler = fn(&crate::world::World) -> String;

/// A name-to-function registry; names cannot silently replace one another.
#[derive(Default)]
pub struct Registry {
    operations: BTreeMap<String, Handler>,
}

impl Registry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one implementation, refusing duplicates.
    pub fn register(&mut self, name: &str, handler: Handler) -> Result<(), String> {
        if name.trim().is_empty() || self.operations.contains_key(name) {
            return Err(format!("invalid or duplicate operation {name:?}"));
        }
        self.operations.insert(name.to_string(), handler);
        Ok(())
    }

    /// Invoke the registered function on the calling thread.
    pub fn invoke(&self, name: &str, world: &crate::world::World) -> Result<String, String> {
        self.operations
            .get(name)
            .map(|handler| handler(world))
            .ok_or_else(|| format!("unknown operation: {name}"))
    }

    /// Whether an implementation is registered for this caller.
    pub fn contains(&self, name: &str) -> bool {
        self.operations.contains_key(name)
    }
}

/// Existing world information, presented for a local operator.
pub fn world_status(world: &crate::world::World) -> String {
    format!(
        "World {}: {} map(s) loaded.",
        world.realm(),
        world.maps().len()
    )
}

/// Build both outputs of the existing map-list operation.
///
/// ASURA: `crates/asura-world/src/chat.rs:41-102,288-324`.
pub fn map_targets(maps: &[crate::world::MapEntry]) -> crate::chat::MappedMaps {
    crate::chat::maps_answer(maps)
}

/// Present the existing map-list operation to a local operator.
pub fn list_targets(world: &crate::world::World) -> String {
    map_targets(world.maps()).table.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_refuses_collisions_and_calls_the_registered_function() {
        let mut registry = Registry::new();
        registry.register("world.status", world_status).unwrap();
        assert!(registry.register("world.status", world_status).is_err());
        assert_eq!(
            registry
                .invoke("world.status", &crate::world::World::new(7))
                .unwrap(),
            "World 7: 0 map(s) loaded."
        );
        assert!(
            registry
                .invoke("missing", &crate::world::World::new(7))
                .is_err()
        );
    }
}
