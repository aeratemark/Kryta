//! Where a character enters, when the map a client names is not a place it can be put in.
//!
//! Reference: `AuthSrv_HandleRequestGameInstance` decides the destination with the character's own
//! row, not with the client's number alone - a client has no business knowing which outpost its
//! character stands in. The rule is ported from the working implementation's rule layer,
//! `ASURA/crates/asura-rules/src/lib.rs` (`placement`), and the two cases ported here are the ones a
//! real client has needed:
//!
//! 1. **A creation visit is its own destination.** `map_id == CreationMap` *and* the asked-for type is
//!    `map_kind::CREATION` is the character-creation instance, and it is not substituted.
//! 2. **The mistaken creation request.** A client that names the creation map as a *place* - which is
//!    what this client does when a character is clicked, `0x8029` with map 0 and type 0 - is placed in
//!    the map its character's row names, then in its last outpost, and only then in the fallback
//!    outpost the caller supplies. Measured 2026-11-12: without this, the login hands the client the
//!    creation instance again and the client waits on a loading screen that never ends.
//!
//! Not ported: the third case, an ask for a destination that is neither a main town nor an outpost -
//! it needs the destination's own type from the world's catalogue, which the login program does not
//! hold (`docs/port/02-wire.md`).

use crate::world::map_kind;

/// What the client asked for, in its own numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requested {
    /// The map the client named.
    pub map_id: u16,
    /// The type it named with it: 0 an outpost, 1 an explorable, 11 the creation instance.
    pub map_type: u32,
}

/// What the character's own row says, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Character {
    /// The map its row names, or the creation map when it names none.
    pub map_id: u16,
    /// The outpost it was last in, or the creation map when it has never been in one.
    pub last_outpost: u16,
}

/// Why the destination is not the one the client named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Substituted {
    /// The client named the creation instance as a place, and the character's row was the answer.
    CreationNamedAsPlace(&'static str),
    /// Neither the row nor the last outpost names a map, so the caller's fallback is the answer.
    NothingToPlace,
}

/// The map the character enters, and why it is not the one the client asked for.
///
/// `fallback_outpost` and `creation_map` are the caller's - both are settings, because which outpost a
/// character with no history enters and which map id means "nothing written yet" are the operator's
/// numbers, not this rule's (`docs/port/04-configuration.md`).
pub fn place(
    requested: Requested,
    character: Character,
    creation_map: u16,
    fallback_outpost: u16,
) -> (u16, Option<Substituted>) {
    // 1. A creation visit is its own destination: the screen has to be reachable.
    if requested.map_id == creation_map && requested.map_type == u32::from(map_kind::CREATION) {
        return (requested.map_id, None);
    }

    // 2. The mistaken creation request: the creation map named as a place to stand.
    if requested.map_id == creation_map {
        if character.map_id != creation_map {
            return (
                character.map_id,
                Some(Substituted::CreationNamedAsPlace("the character's row")),
            );
        }
        if character.last_outpost != creation_map {
            return (
                character.last_outpost,
                Some(Substituted::CreationNamedAsPlace("its last outpost")),
            );
        }
        return (fallback_outpost, Some(Substituted::NothingToPlace));
    }

    // 3. Every other destination is the client's own, until the catalogue's rule for a destination
    // that is not a main town arrives with the world's map kinds.
    (requested.map_id, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The creation instance's map id, which the world's own catalogue names.
    const CREATION: u16 = 0;
    /// The outpost a character with no history enters, as the auth program's setting states it.
    const FALLBACK: u16 = 148;

    #[test]
    fn a_creation_visit_reaches_the_creation_instance() {
        let requested = Requested {
            map_id: CREATION,
            map_type: u32::from(map_kind::CREATION),
        };
        let fresh = Character {
            map_id: CREATION,
            last_outpost: CREATION,
        };
        assert_eq!(place(requested, fresh, CREATION, FALLBACK), (CREATION, None));
    }

    #[test]
    fn a_character_click_that_names_the_creation_map_enters_the_characters_own_map() {
        // What this client really sends when a character is clicked: map 0, type 0, district 0 and
        // language -1 (measured 2026-11-12, `Auth.log`). The row is the answer.
        let requested = Requested {
            map_id: CREATION,
            map_type: 0,
        };
        let played = Character {
            map_id: 148,
            last_outpost: 148,
        };
        assert_eq!(
            place(requested, played, CREATION, FALLBACK),
            (
                148,
                Some(Substituted::CreationNamedAsPlace("the character's row"))
            )
        );
    }

    #[test]
    fn a_row_that_names_no_map_falls_back_to_its_last_outpost() {
        let requested = Requested {
            map_id: CREATION,
            map_type: 0,
        };
        let somewhere = Character {
            map_id: CREATION,
            last_outpost: 148,
        };
        assert_eq!(
            place(requested, somewhere, CREATION, FALLBACK),
            (
                148,
                Some(Substituted::CreationNamedAsPlace("its last outpost"))
            )
        );
    }

    #[test]
    fn a_character_that_has_never_been_anywhere_gets_the_callers_fallback() {
        let requested = Requested {
            map_id: CREATION,
            map_type: 0,
        };
        let fresh = Character {
            map_id: CREATION,
            last_outpost: CREATION,
        };
        assert_eq!(
            place(requested, fresh, CREATION, FALLBACK),
            (FALLBACK, Some(Substituted::NothingToPlace))
        );
    }

    #[test]
    fn a_place_the_client_names_that_is_not_the_creation_instance_is_left_alone() {
        let requested = Requested {
            map_id: 473,
            map_type: 0,
        };
        let played = Character {
            map_id: 148,
            last_outpost: 148,
        };
        assert_eq!(place(requested, played, CREATION, FALLBACK), (473, None));
    }
}
