//! `game/Districts/` - the reference's `src/server/game/Instances/` (4 files).
//!
//! Renamed: the reference calls this directory `Instances/`.
//!
//! What is here now is the part the load needs and the ledger states: an outpost is **one running
//! copy per district** (R1, R4); the first district opens when an outpost is entered and the next
//! when one fills its allotment (R5, R16); a joining character is placed in the first district with
//! room (R22) unless the travel named one, in which case it is honoured (R57) - and a district that
//! is full cannot be entered (R58). The allotment is a setting, never a constant (R5); it comes from
//! `game/World/WorldConfig.rs` at the moment a character enters.
//!
//! An explorable is **private per party** (R3): with no parties yet, one connection is one party, so
//! each entry opens its own copy.
//!
//! What is **not** here yet: the instance's own row and its lifecycle - the 15-minute empty hold
//! (R37, R38) and disposal (R40). Those need the instance table and the tick, and arrive with it.

use crate::world::map_kind::OUTPOST;
use crate::world::MapEntry;

/// One running copy of a map: which map, which district number, who is inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Copy {
    map_id: u32,
    district: u32,
    members: u32,
}

/// The live copies, and the district numbers they hold.
///
/// One world owns this: a copy is the world's state, not a session's.
#[derive(Debug, Clone)]
pub struct Districts {
    copies: Vec<Copy>,
}

impl Default for Districts {
    fn default() -> Self {
        Self::new()
    }
}

impl Districts {
    /// A world with no copy open.
    pub fn new() -> Self {
        Self { copies: Vec::new() }
    }

    /// Which district a joining character enters, and the copy it enters.
    ///
    /// The rules, in the order they act:
    /// - a named district (`1..=n`) is honoured - an outpost's copies are numbered, and a number the
    ///   world does not hold yet is opened as that district (R4, R57);
    /// - a copy whose members have reached the allotment cannot be entered (R58), and a request for
    ///   one is refused rather than moved: the client asked for that district;
    /// - a request with no district takes the first copy with room (R22), and when every copy is full
    ///   the next one is opened (R5, R16).
    pub fn enter(
        &mut self,
        map: &MapEntry,
        district_hint: u32,
        allotment: u32,
    ) -> Result<u32, String> {
        if map.kind != OUTPOST {
            // R3: an explorable copy belongs to one party. Without parties yet, one connection is
            // one party, so it gets a copy of its own - numbered after the copies this world already
            // holds for the map, which is what the client shows as districts.
            let district = self.next_number(map.id);
            self.copies.push(Copy {
                map_id: map.id,
                district,
                members: 1,
            });
            return Ok(district);
        }

        let allotment = allotment.max(1);
        if district_hint >= 1 {
            match self.copy_mut(map.id, district_hint) {
                Some(copy) => {
                    if copy.members >= allotment {
                        return Err(format!(
                            "district {} of map {} is full ({} of {}) and cannot be entered",
                            district_hint, map.id, copy.members, allotment
                        ));
                    }
                    copy.members += 1;
                    return Ok(district_hint);
                }
                None => {
                    self.copies.push(Copy {
                        map_id: map.id,
                        district: district_hint,
                        members: 1,
                    });
                    return Ok(district_hint);
                }
            }
        }

        if let Some(copy) = self.first_with_room(map.id, allotment) {
            copy.members += 1;
            return Ok(copy.district);
        }

        // R5: the outpost is full, so the next district opens - the number after the highest held.
        let district = self.next_number(map.id);
        self.copies.push(Copy {
            map_id: map.id,
            district,
            members: 1,
        });
        Ok(district)
    }

    /// One connection ends, and its place is given back.
    ///
    /// An outpost that empties stays open - R37 holds it for 15 minutes of inactivity - and the copy
    /// is disposed by the lifecycle, not here. What this guarantees is the count: the district has
    /// one fewer member, and a copy with no members is a candidate for that hold.
    pub fn leave(&mut self, map_id: u32, district: u32) {
        if let Some(copy) = self.copy_mut(map_id, district) {
            copy.members = copy.members.saturating_sub(1);
        }
    }

    /// How many members a copy holds, for a test or a status line.
    pub fn members(&self, map_id: u32, district: u32) -> u32 {
        self.copies
            .iter()
            .find(|copy| copy.map_id == map_id && copy.district == district)
            .map_or(0, |copy| copy.members)
    }

    /// How many copies a map has open.
    pub fn count_of(&self, map_id: u32) -> usize {
        self.copies
            .iter()
            .filter(|copy| copy.map_id == map_id)
            .count()
    }

    /// The districts this world holds open, in the order they were opened - for a log line.
    pub fn open(&self) -> Vec<(u32, u32, u32)> {
        self.copies
            .iter()
            .map(|copy| (copy.map_id, copy.district, copy.members))
            .collect()
    }

    fn copy_mut(&mut self, map_id: u32, district: u32) -> Option<&mut Copy> {
        self.copies
            .iter_mut()
            .find(|copy| copy.map_id == map_id && copy.district == district)
    }

    /// The lowest-numbered copy of a map with room left (R22).
    fn first_with_room(&mut self, map_id: u32, allotment: u32) -> Option<&mut Copy> {
        self.copies
            .iter_mut()
            .filter(|copy| copy.map_id == map_id && copy.members < allotment)
            .min_by_key(|copy| copy.district)
    }

    /// The next district number a map would open: one past the highest it holds.
    fn next_number(&self, map_id: u32) -> u32 {
        self.copies
            .iter()
            .filter(|copy| copy.map_id == map_id)
            .map(|copy| copy.district)
            .max()
            .map_or(1, |highest| highest + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map_kind;
    use crate::world::MapEntry;

    fn outpost() -> MapEntry {
        MapEntry {
            id: 148,
            kind: OUTPOST,
            file_id: 112_509,
            name: "Ascalon City (pre-Searing)".to_string(),
        }
    }

    fn explorable() -> MapEntry {
        MapEntry {
            id: 5,
            kind: map_kind::EXPLORABLE,
            file_id: 1,
            name: "Regent Valley".to_string(),
        }
    }

    #[test]
    fn the_first_entry_opens_district_one() {
        let mut districts = Districts::new();
        assert_eq!(districts.enter(&outpost(), 0, 100).unwrap(), 1);
        assert_eq!(districts.count_of(148), 1);
        assert_eq!(districts.members(148, 1), 1);
    }

    #[test]
    fn a_named_district_is_honoured_even_before_it_exists() {
        let mut districts = Districts::new();
        assert_eq!(districts.enter(&outpost(), 3, 100).unwrap(), 3);
        assert_eq!(districts.members(148, 3), 1);
        assert_eq!(districts.count_of(148), 1);
    }

    #[test]
    fn a_filled_district_is_left_for_the_first_with_room() {
        let mut districts = Districts::new();
        districts.enter(&outpost(), 0, 2).unwrap();
        districts.enter(&outpost(), 0, 2).unwrap();
        // District 1 is full (2 of 2): the next joining character gets the next district (R5).
        assert_eq!(districts.enter(&outpost(), 0, 2).unwrap(), 2);
        assert_eq!(districts.count_of(148), 2);
        assert_eq!(districts.members(148, 1), 2);
        assert_eq!(districts.members(148, 2), 1);
    }

    #[test]
    fn a_named_full_district_is_refused() {
        let mut districts = Districts::new();
        districts.enter(&outpost(), 1, 1).unwrap();
        let refusal = districts.enter(&outpost(), 1, 1).unwrap_err();
        assert!(
            refusal.contains("district 1 of map 148 is full"),
            "{refusal}"
        );
    }

    #[test]
    fn leaving_gives_the_place_back() {
        let mut districts = Districts::new();
        districts.enter(&outpost(), 0, 2).unwrap();
        districts.enter(&outpost(), 0, 2).unwrap();
        districts.leave(148, 1);
        assert_eq!(districts.members(148, 1), 1);
        // The place that opened is the one the next entry takes.
        assert_eq!(districts.enter(&outpost(), 0, 2).unwrap(), 1);
    }

    #[test]
    fn an_explorable_gets_a_copy_of_its_own() {
        let mut districts = Districts::new();
        assert_eq!(districts.enter(&explorable(), 0, 100).unwrap(), 1);
        assert_eq!(districts.enter(&explorable(), 0, 100).unwrap(), 2);
        assert_eq!(districts.count_of(5), 2);
        assert_eq!(districts.members(5, 1), 1);
    }
}
