//! The view models the pages draw: an account with its roster count, a character as the roster
//! lists it, and the game's own names for the numbers a row stores.
//!
//! These are built from the nouns' repositories (`game::accounts`, `game::characters`) - the same
//! rows the login, the world and the tool read - so a page never owns a second copy of the data.

use std::collections::HashMap;

use game::characters::Character;

use crate::Manage;

/// The campaign a character was made in, as the client and the row's own comment name it.
/// `characters.campaign`: 0 none (a PvP character), 1 Prophecies, 2 Factions, 3 Nightfall,
/// 4 Eye of the North (`data/sql/base/db_characters/characters.sql`).
pub const CAMPAIGNS: [(u8, &str); 5] = [
    (0, "None (PvP)"),
    (1, "Prophecies"),
    (2, "Factions"),
    (3, "Nightfall"),
    (4, "Eye of the North"),
];

/// The professions, in the client's own numbering.
pub const PROFESSIONS: [(u8, &str); 11] = [
    (0, "None"),
    (1, "Warrior"),
    (2, "Ranger"),
    (3, "Monk"),
    (4, "Necromancer"),
    (5, "Mesmer"),
    (6, "Elementalist"),
    (7, "Assassin"),
    (8, "Ritualist"),
    (9, "Paragon"),
    (10, "Dervish"),
];

/// The name of a campaign, or `Unknown` for a number this tree has no name for.
pub fn campaign_label(value: u8) -> &'static str {
    CAMPAIGNS
        .iter()
        .find(|(id, _)| *id == value)
        .map(|(_, name)| *name)
        .unwrap_or("Unknown")
}

/// The name of a profession, or `Unknown` for a number this tree has no name for.
pub fn profession_label(value: u8) -> &'static str {
    PROFESSIONS
        .iter()
        .find(|(id, _)| *id == value)
        .map(|(_, name)| *name)
        .unwrap_or("Unknown")
}

/// The profession `<option>` list, one selected.
pub fn profession_options(selected: u8) -> String {
    PROFESSIONS
        .iter()
        .skip(1)
        .map(|(id, name)| {
            let selected = if *id == selected { " selected" } else { "" };
            format!("<option value=\"{id}\"{selected}>{name}</option>")
        })
        .collect()
}

/// A character's profession pair, as the character page and the roster read it.
pub fn professions(primary: u8, secondary: u8) -> String {
    if secondary == 0 {
        profession_label(primary).to_string()
    } else {
        format!(
            "{} / {}",
            profession_label(primary),
            profession_label(secondary)
        )
    }
}

/// The name of a map an id names, when the world pack answered.
pub fn map_names(manage: &Manage) -> HashMap<u32, String> {
    manage.map_names()
}

/// Where a character is, as the roster lists it: the outpost's name, its id, or "not placed".
pub fn outpost_label(map_id: u16, names: &HashMap<u32, String>) -> String {
    if map_id == 0 {
        return "not placed".to_string();
    }
    match names.get(&u32::from(map_id)) {
        Some(name) => format!("{} ({})", name, map_id),
        None => format!("map {map_id}"),
    }
}

/// One account line of the directory, with the number of players it owns.
pub struct AccountRow {
    /// The row itself.
    pub account: game::accounts::Account,
    /// How many players (characters) it owns; 0 when no character database is configured.
    pub players: usize,
}

/// Every account, with its player count, oldest first.
pub fn accounts(manage: &Manage) -> Result<Vec<AccountRow>, String> {
    let accounts = manage.list_accounts()?;
    let counts = manage
        .characters()
        .and_then(|characters| characters.counts_by_account().ok())
        .unwrap_or_default();
    Ok(accounts
        .into_iter()
        .map(|account| AccountRow {
            players: counts.get(&account.user_id).copied().unwrap_or(0),
            account,
        })
        .collect())
}

/// One player as a roster lists it: a character, which belongs to one account.
pub struct PlayerRow {
    /// The name the client shows.
    pub name: String,
    /// The id the client knows the player by.
    pub uuid: String,
    /// Its campaign's name.
    pub campaign: String,
    /// Its profession pair.
    pub professions: String,
    /// Its level.
    pub level: u8,
    /// The outpost it last stood in.
    pub outpost: String,
    /// Whether it is a PvP character.
    pub pvp: bool,
}

impl PlayerRow {
    /// One row of a players table.
    pub fn of(character: &Character, names: &HashMap<u32, String>) -> Self {
        // The primary profession is packed in the appearance word, bits 20-23 (`docs/game-rules.md`
        // E74); the second is its own column.
        let primary = ((character.settings.appearance >> 20) & 0xF) as u8;
        Self {
            name: character.name.clone(),
            uuid: game::characters::uuid_text(&character.uuid),
            campaign: campaign_label(character.settings.campaign).to_string(),
            professions: professions(primary, character.settings.secondary_profession),
            level: character.settings.level,
            outpost: outpost_label(character.settings.last_outpost, names),
            pvp: character.settings.is_pvp,
        }
    }
}

/// The fields packed into the appearance word, in the order the client packs them
/// (`docs/game-rules.md` E74).
pub fn appearance_fields(word: u32) -> [(&'static str, u32); 6] {
    [
        ("sex", word & 0x1),
        ("height", (word >> 1) & 0xF),
        ("skin", (word >> 5) & 0x1F),
        ("hair colour", (word >> 10) & 0x1F),
        ("face", (word >> 15) & 0x1F),
        ("hair style", (word >> 24) & 0x3F),
    ]
}

/// The sex the appearance word's first bit names.
pub fn sex_label(word: u32) -> &'static str {
    if word & 0x1 == 1 { "female" } else { "male" }
}

/// An item's name, from the UTF-16 code units a held item carries.
pub fn item_name(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_numbers_have_their_names() {
        assert_eq!(campaign_label(1), "Prophecies");
        assert_eq!(campaign_label(3), "Nightfall");
        assert_eq!(campaign_label(9), "Unknown");
        assert_eq!(profession_label(7), "Assassin");
        assert_eq!(profession_label(10), "Dervish");
        assert_eq!(professions(1, 0), "Warrior");
        assert_eq!(professions(6, 3), "Elementalist / Monk");
    }

    #[test]
    fn the_appearance_word_is_the_clients_own_bits() {
        // sex 1, height 7, skin 3, hair colour 9, face 12, hair style 33.
        let word = 1 | (7 << 1) | (3 << 5) | (9 << 10) | (12 << 15) | (33 << 24);
        let fields = appearance_fields(word);
        assert_eq!(fields[0], ("sex", 1));
        assert_eq!(fields[1], ("height", 7));
        assert_eq!(fields[2], ("skin", 3));
        assert_eq!(fields[3], ("hair colour", 9));
        assert_eq!(fields[4], ("face", 12));
        assert_eq!(fields[5], ("hair style", 33));
        assert_eq!(sex_label(word), "female");
    }
}
