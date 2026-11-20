//! The players: one account's roster, and one character with the data that belongs to it.
//!
//! A character is the account's, and what it owns is its own: where it stands, what it carries,
//! what it has unlocked, and (when this game records them) its titles. The roster is the login's own
//! list read back; writing one is the creation screen's job in the game and this page's explicit
//! act here, and every edit says it does not reach an online character.

use shared::network::http::{Request, Response};

use crate::view;
use crate::web::{components, http, layout};
use crate::Manage;

/// The creation screen's own list of campaigns: the three a role-playing character may choose.
/// A PvP character (campaign 0) is refused because the world pack names no PvP starting map.
const CREATION_CAMPAIGNS: [u8; 3] = [1, 2, 3];

/// An account's roster: every character it owns, and the form that makes one.
pub fn roster(manage: &Manage, account_id: i64) -> Response {
    let account = match find_account(manage, account_id) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let characters = match characters_of(manage, &account) {
        Ok(characters) => characters,
        Err(response) => return response,
    };
    let names = view::map_names(manage);
    let rows = characters
        .iter()
        .map(|character| {
            let row = view::CharacterRow::of(character, &names);
            let href = format!("/players/{account_id}/characters/{}", row.uuid);
            components::row(&[
                format!("<a href=\"{href}\">{}</a>", components::escape(&row.name)),
                format!("<code>{}</code>", components::escape(&row.uuid)),
                row.level.to_string(),
                components::escape(&row.campaign),
                components::escape(&row.professions),
                components::escape(&row.outpost),
                if row.pvp { "yes" } else { "no" }.to_string(),
            ])
        })
        .collect::<Vec<_>>();
    let table_body = if rows.is_empty() {
        components::note("No character exists on this account yet.")
    } else {
        components::table(
            &["player", "id", "level", "campaign", "professions", "last outpost", "PvP"],
            &rows,
        )
    };

    let tabs = tabs(account_id, "characters");
    let body = format!(
        "{crumbs}\n{tabs}\
         <p class=\"note\">Every row of the character table, as the login's own list reads them:\
         a name, the id the client knows it by, its level, campaign, profession pair, last outpost\
         and its PvP flag. Its page owns the character's progress, position, inventory, titles and\
         unlocks.</p>\n\
         {table}\n\
         <p class=\"note\">{count} character(s).</p>\n\
         {create}",
        crumbs = components::crumbs(&[
            ("/players", "Players"),
            ("", &components::escape(&account.username)),
            ("", "Characters"),
        ]),
        tabs = components::section_nav(&tabs),
        table = table_body,
        count = characters.len(),
        create = create_form(account_id),
    );
    layout::page("Players", "players", &body)
}

/// One character: its own data, and the edits an operator may make.
pub fn detail(manage: &Manage, account_id: i64, uuid: &str) -> Response {
    let account = match find_account(manage, account_id) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let characters = match characters_of(manage, &account) {
        Ok(characters) => characters,
        Err(response) => return response,
    };
    let Some(character) = characters
        .iter()
        .find(|character| game::characters::uuid_text(&character.uuid) == uuid)
    else {
        return crate::pages::error(404, "players", "Character not found on this account.");
    };
    let uuid = game::characters::uuid_text(&character.uuid);
    let names = view::map_names(manage);
    let row = view::CharacterRow::of(character, &names);
    let sheet = manage
        .characters()
        .map(|characters| characters.sheet(&uuid, character.uuid));
    let gold = sheet.as_ref().map(|sheet| sheet.gold).unwrap_or(0);
    let primary = ((character.settings.appearance >> 20) & 0xF) as u8;
    let position = manage
        .characters()
        .and_then(|characters| characters.position(&uuid).ok().flatten());
    let position_text = match position {
        Some(position) => format!(
            "x {:.2} y {:.2} on plane {} in {}",
            position.x,
            position.y,
            position.plane,
            view::outpost_label(position.map_id as u16, &names)
        ),
        None => "never saved - the character has not been placed since it was played".to_string(),
    };

    let inventory = match manage.list_items_of(&uuid) {
        Some(items) if items.is_empty() => components::note("The character carries nothing."),
        Some(items) => {
            let rows = items
                .iter()
                .map(|item| {
                    let name = crate::view::item_name(&item.name);
                    let name = if name.trim().is_empty() {
                        "(unnamed)".to_string()
                    } else {
                        name
                    };
                    components::row(&[
                        components::escape(&name),
                        item.bag_id.to_string(),
                        item.slot.to_string(),
                        item.quantity.to_string(),
                        format!("<code>{}</code>", item.file_id),
                        item.model_id.to_string(),
                    ])
                })
                .collect::<Vec<_>>();
            components::table(&["item", "bag", "slot", "quantity", "file id", "model"], &rows)
        }
        None => components::note(
            "No character database is configured in manage.json, so the inventory cannot be read.",
        ),
    };

    let unlocks = match manage.list_unlocks(&uuid) {
        Some(maps) if maps.is_empty() => {
            components::note("The character has no unlocked map yet (R12).")
        }
        Some(maps) => {
            let cells = maps
                .iter()
                .map(|map_id| match names.get(map_id) {
                    Some(name) => format!("<code>{map_id}</code> {}", components::escape(name)),
                    None => format!("<code>{map_id}</code>"),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("<p>{cells}</p>")
        }
        None => components::note(
            "No character database is configured in manage.json, so the unlocks cannot be read.",
        ),
    };

    let tabs = tabs(account_id, "characters");
    let name = components::escape(&character.name);
    let body = format!(
        "{crumbs}\n{tabs}\
         <section class=\"panel\"><div class=\"panel-head\"><div><h2>{name}</h2>\
         <p>Character id <code>{uuid}</code></p></div></div>\
         <table><tbody>\
         <tr><th>campaign</th><td>{campaign}</td></tr>\
         <tr><th>professions</th><td>{professions} (primary {primary})</td></tr>\
         <tr><th>level</th><td>{level}</td></tr>\
         <tr><th>gold</th><td>{gold}</td></tr>\
         <tr><th>PvP</th><td>{pvp}</td></tr>\
         <tr><th>last outpost</th><td>{outpost}</td></tr>\
         <tr><th>position</th><td>{position}</td></tr>\
         <tr><th>appearance</th><td>{appearance}</td></tr>\
         </tbody></table>\
         <p class=\"note\">Changes are not synchronized with an online character. Log the character\
         out before editing or deleting it.</p>\
         <div class=\"field-grid\">\
         <form class=\"editor-form\" method=\"post\" action=\"/players/{account_id}/characters/{uuid}/rename\">\
         {rename_field}<div class=\"actions\">{rename_button}</div></form>\
         <form class=\"editor-form\" method=\"post\" action=\"/players/{account_id}/characters/{uuid}/level\">\
         {level_field}<div class=\"actions\">{level_button}</div></form>\
         </div></section>\n\
         <section class=\"panel\"><h2>Inventory and equipment</h2>\
         <p>The item rows the character's load draws from, with the bag and slot each sits in.</p>{inventory}</section>\n\
         <section class=\"panel\"><h2>Titles</h2>\
         <p>Titles belong to the character in the game: a record per track, its points, the current\
         and next tier, and the one the player shows. This server does not record titles yet - the\
         title messages are evidenced (E23, E156-E158) but no table or handler exists - so there is\
         nothing to show or edit here. Nothing is hidden or zeroed to pretend otherwise.</p></section>\n\
         <section class=\"panel\"><h2>Unlocked maps</h2>\
         <p>Where a character may travel (R12), as the load's gate reads it.</p>{unlocks}</section>\n\
         <section class=\"panel\"><div class=\"panel-head\"><div><h2>Permanent removal</h2>\
         <p>Removes the character's row and the records that are its own: its items and its\
         unlocks.</p></div></div>\
         <form method=\"post\" action=\"/players/{account_id}/characters/{uuid}/delete\" \
         onsubmit=\"return confirm('Remove {confirm} and its items and unlocks?')\">{remove}</form></section>",
        crumbs = components::crumbs(&[
            ("/players", "Players"),
            ("", &components::escape(&account.username)),
            ("", "Characters"),
            ("", &name),
        ]),
        tabs = components::section_nav(&tabs),
        name = name,
        uuid = components::escape(&uuid),
        campaign = components::escape(&row.campaign),
        professions = components::escape(&row.professions),
        primary = view::profession_label(primary),
        level = row.level,
        gold = gold,
        pvp = if row.pvp { "yes" } else { "no" },
        outpost = components::escape(&row.outpost),
        position = components::escape(&position_text),
        appearance = {
            let fields = view::appearance_fields(character.settings.appearance);
            let sex = view::sex_label(character.settings.appearance);
            format!(
                "{sex}, height {}, skin {}, hair colour {}, face {}, hair style {}",
                fields[1].1, fields[2].1, fields[3].1, fields[4].1, fields[5].1
            )
        },
        account_id = account_id,
        rename_field = components::field(
            "Name",
            &components::text_input("name", &character.name)
        ),
        rename_button = components::submit("Rename"),
        level_field = components::field(
            "Level (1 to 20, R48)",
            &components::number_input("level", row.level, 1, 20)
        ),
        level_button = components::submit("Save level"),
        inventory = inventory,
        unlocks = unlocks,
        confirm = name,
        remove = components::submit_class("Remove character", "danger"),
    );
    layout::page("Players", "players", &body)
}

/// `POST /players/{id}/characters`: write a character from the creation record lists.
pub fn create(manage: &Manage, request: &Request, account_id: i64) -> Response {
    let account = match find_account(manage, account_id) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let form = http::form_fields(&request.body_text());
    let run = || -> Result<(), String> {
        let characters = manage
            .characters()
            .ok_or("The character database is not configured.")?;
        let name = http::value(&form, "name").trim();
        if !valid_character_name(name) {
            return Err("Name must be 3 to 19 English letters, a first name and a surname.".into());
        }
        let number = |key: &str, max: u32| -> Result<u32, String> {
            let value = http::value(&form, key)
                .parse::<u32>()
                .map_err(|_| format!("Invalid {key}."))?;
            if value > max {
                return Err(format!("Invalid {key}."));
            }
            Ok(value)
        };
        let campaign = number("campaign", 3)? as u8;
        if campaign == 0 {
            return Err("PvP creation is unavailable: the world pack names no PvP starting map.".into());
        }
        let profession = number("profession", 10)? as u8;
        let valid = (1..=6).contains(&profession)
            || (campaign == 2 && (7..=8).contains(&profession))
            || (campaign == 3 && (9..=10).contains(&profession));
        if !valid {
            return Err("This profession is unavailable in the selected campaign.".into());
        }
        // docs/game-rules.md E74: bit positions in the client's appearance word.
        let appearance = number("sex", 1)?
            | (number("height", 15)? << 1)
            | (number("skin", 31)? << 5)
            | (number("hair_color", 31)? << 10)
            | (number("face", 31)? << 15)
            | (u32::from(profession) << 20)
            | (number("hair_style", 63)? << 24)
            | (u32::from(campaign) << 30);

        let world_config = manage
            .world_config()
            .ok_or("The world program's settings are not named in manage.json.")?;
        let config = common::configuration::Config::load_json(
            world_config,
            false,
            common::configuration::Policy::default(),
        )
        .map_err(|error| error.to_string())?;
        let (inventory_path, _) = config.value_string("InventoryFile", "").map_err(|e| e.to_string())?;
        let (equipment_path, _) = config.value_string("EquipmentFile", "").map_err(|e| e.to_string())?;
        let (worn_path, _) = config.value_string("EquippedSlotsFile", "").map_err(|e| e.to_string())?;
        let (pack_path, _) = config.value_string("WorldPack", "").map_err(|e| e.to_string())?;
        let log = manage.log();
        let tables = game::creation::Tables::load(
            &inventory_path,
            &equipment_path,
            Some(std::path::Path::new(&worn_path)),
            log,
        )?;
        if tables.set(campaign, profession).is_none() {
            return Err("The world pack has no equipment for this choice.".into());
        }

        let spawn_path = std::path::Path::new(&pack_path).join("character_spawn.json");
        let spawn_data: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&spawn_path).map_err(|error| format!("{}: {error}", spawn_path.display()))?,
        )
        .map_err(|error| error.to_string())?;
        let map = spawn_data["spawns"]
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|row| row["campaign"].as_u64() == Some(u64::from(campaign)))
            })
            .and_then(|row| row["map_id"].as_u64())
            .and_then(|id| u16::try_from(id).ok())
            .ok_or("The world pack names no starting map for this campaign.")?;
        let pack = game::maps::pack::Pack::load(&pack_path, log)?;
        let (maps, _) = pack.load_maps()?;
        if !maps
            .iter()
            .any(|entry| entry.id == u32::from(map) && entry.is_hostable())
        {
            return Err(format!("The world pack cannot host starting map {map}."));
        }

        let templates = manage
            .items()
            .ok_or("The world database is not configured, so item templates are not available.")?
            .list()?;
        let mut id = [0u8; 16];
        getrandom::getrandom(&mut id).map_err(|error| format!("Cannot create a character id: {error}"))?;
        let uuid = game::characters::uuid_text(&id);
        if !characters.insert_creation(
            &uuid,
            &account.user_id,
            name,
            appearance,
            profession,
            campaign,
            map,
        )? {
            return Err("That character name is already in use.".into());
        }
        let initialized = characters
            .write_starting_items(&uuid, &tables.inventory(campaign, profession), &templates)
            .and_then(|_| characters.unlock_map(&uuid, u32::from(map)).map(|_| ()));
        if let Err(error) = initialized {
            return match characters.remove(&uuid) {
                Ok(_) => Err(format!("Creation failed and was rolled back: {error}")),
                Err(cleanup) => Err(format!(
                    "Creation failed: {error}; cleanup failed for {uuid}: {cleanup}"
                )),
            };
        }
        manage.say(format!("account {account_id}: created character {uuid}"));
        Ok(())
    };
    match run() {
        Ok(()) => http::redirect(&format!("/players/{account_id}/characters")),
        Err(error) => crate::pages::error(400, "players", &error),
    }
}

/// `POST /players/{id}/characters/{uuid}/rename`: rename only a character on this account.
pub fn rename(manage: &Manage, request: &Request, account_id: i64, uuid: &str) -> Response {
    let account = match find_account(manage, account_id) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let form = http::form_fields(&request.body_text());
    let name = http::value(&form, "name").trim();
    if !valid_character_name(name) {
        return crate::pages::error(400, "players", "Name must be 3 to 19 English letters, a first name and a surname.");
    }
    let Some(characters) = manage.characters() else {
        return crate::pages::error(503, "players", "No character database is configured.");
    };
    match characters.rename_for_account(&account.user_id, uuid, name) {
        Ok(true) => {
            manage.say(format!("account {account_id}: renamed character {uuid}"));
            http::redirect(&format!("/players/{account_id}/characters/{uuid}"))
        }
        Ok(false) => crate::pages::error(404, "players", "Character not found on this account."),
        Err(error) => crate::pages::error(500, "players", &format!("The character database said: {error}")),
    }
}

/// `POST /players/{id}/characters/{uuid}/level`: set a level in the game's own range (R48).
pub fn set_level(manage: &Manage, request: &Request, account_id: i64, uuid: &str) -> Response {
    let account = match find_account(manage, account_id) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let form = http::form_fields(&request.body_text());
    let level = match http::value(&form, "level").parse::<u8>() {
        Ok(level) if (1..=20).contains(&level) => level,
        _ => {
            return crate::pages::error(
                400,
                "players",
                "A level is a number from 1 to 20, the game's own range (R48).",
            );
        }
    };
    let Some(characters) = manage.characters() else {
        return crate::pages::error(503, "players", "No character database is configured.");
    };
    if !characters
        .for_account(&account.user_id)
        .iter()
        .any(|character| game::characters::uuid_text(&character.uuid) == uuid)
    {
        return crate::pages::error(404, "players", "Character not found on this account.");
    }
    match characters.set_level(uuid, level) {
        Ok(true) => {
            manage.say(format!("character {uuid}: level {level}"));
            http::redirect(&format!("/players/{account_id}/characters/{uuid}"))
        }
        Ok(false) => crate::pages::error(404, "players", "The character is already gone."),
        Err(error) => crate::pages::error(500, "players", &format!("The character database said: {error}")),
    }
}

/// `POST /players/{id}/characters/{uuid}/delete`: remove the character and the records its own.
pub fn delete(manage: &Manage, request: &Request, account_id: i64, uuid: &str) -> Response {
    let _ = request;
    let account = match find_account(manage, account_id) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let Some(characters) = manage.characters() else {
        return crate::pages::error(503, "players", "No character database is configured.");
    };
    if !characters
        .for_account(&account.user_id)
        .iter()
        .any(|character| game::characters::uuid_text(&character.uuid) == uuid)
    {
        return crate::pages::error(404, "players", "Character not found on this account.");
    }
    match characters.remove(uuid) {
        Ok(true) => {
            manage.say(format!("character {uuid}: removed, with its items and unlocks"));
            http::redirect(&format!("/players/{account_id}/characters"))
        }
        Ok(false) => crate::pages::error(404, "players", "The character is already gone."),
        Err(error) => crate::pages::error(500, "players", &format!("The character database said: {error}")),
    }
}

/// The account, or the answer a page that cannot find it gives.
fn find_account(manage: &Manage, id: i64) -> Result<game::accounts::Account, Response> {
    match manage.list_accounts() {
        Ok(accounts) => accounts
            .into_iter()
            .find(|account| account.id == id)
            .ok_or_else(|| crate::pages::error(404, "players", "No account has that id.")),
        Err(error) => Err(crate::pages::error(
            500,
            "players",
            &format!("The account table could not be read: {error}"),
        )),
    }
}

/// The account's characters, or the answer a page that cannot read them gives.
fn characters_of(
    manage: &Manage,
    account: &game::accounts::Account,
) -> Result<Vec<game::characters::Character>, Response> {
    match manage.characters_of_account(&account.user_id) {
        Some(characters) => Ok(characters),
        None => Err(crate::pages::error(
            503,
            "players",
            "No character database is configured in manage.json, so there is nothing to list.",
        )),
    }
}

/// The pages an account's own sections link to, with the one being shown marked.
fn tabs(account_id: i64, current: &str) -> Vec<String> {
    vec![
        components::tab(
            Some(&format!("/players/{account_id}")),
            "Overview",
            current == "overview",
        ),
        components::tab(
            Some(&format!("/players/{account_id}/characters")),
            "Characters",
            current == "characters",
        ),
        components::tab(None, "Shared storage", false),
        components::tab(None, "Account unlocks", false),
    ]
}

/// The creation form, drawn from the creation record's own choices.
fn create_form(account_id: i64) -> String {
    let campaigns = view::CAMPAIGNS
        .iter()
        .filter(|(id, _)| CREATION_CAMPAIGNS.contains(id))
        .map(|(id, name)| format!("<option value=\"{id}\">{name}</option>"))
        .collect::<String>();
    format!(
        "<section class=\"panel\"><h2>Create character</h2>\
         <p>Choose a campaign, profession and appearance. PvP creation is unavailable until the\
         world pack names its starting map. Slot capacity is not yet enforced by this server.</p>\
         <form class=\"editor-form\" method=\"post\" action=\"/players/{account_id}/characters\">\
         <div class=\"field-grid\">\
         {name}{campaign}\
         </div><div class=\"field-grid\">\
         {profession}{sex}\
         </div><div class=\"field-grid\">\
         {height}{skin}\
         </div><div class=\"field-grid\">\
         {hair_color}{face}\
         </div><div class=\"field-grid\">\
         {hair_style}{empty}\
         </div>\
         <div class=\"actions\">{button}</div></form></section>",
        name = components::field(
            "Name",
            &components::text_input_placeholder("name", "First Last")
        ),
        campaign = components::field(
            "Campaign",
            &format!("<select name=\"campaign\">{campaigns}</select>")
        ),
        profession = components::field(
            "Profession",
            &format!("<select name=\"profession\">{}</select>", view::profession_options(1))
        ),
        sex = components::field(
            "Sex",
            "<select name=\"sex\"><option value=\"0\">Male</option><option value=\"1\">Female</option></select>"
        ),
        height = components::field("Height (0-15)", &components::number_input("height", 0, 0, 15)),
        skin = components::field("Skin (0-31)", &components::number_input("skin", 0, 0, 31)),
        hair_color = components::field("Hair colour (0-31)", &components::number_input("hair_color", 0, 0, 31)),
        face = components::field("Face (0-31)", &components::number_input("face", 0, 0, 31)),
        hair_style = components::field("Hair style (0-63)", &components::number_input("hair_style", 3, 0, 63)),
        empty = "",
        button = components::submit("Create character"),
    )
}

/// `docs/game-rules.md` R65: a first and last name made of English letters.
pub fn valid_character_name(name: &str) -> bool {
    let words: Vec<_> = name.split(' ').collect();
    (3..=19).contains(&name.len())
        && words.len() >= 2
        && words
            .iter()
            .all(|word| !word.is_empty() && word.bytes().all(|c| c.is_ascii_alphabetic()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_name_is_the_ledgers_own_format() {
        assert!(valid_character_name("Asura Warrior"));
        assert!(!valid_character_name("Asura"));
        assert!(!valid_character_name("Asura  Warrior"));
        assert!(!valid_character_name("Asura Warrior2"));
        assert!(!valid_character_name("An Excessively Long Character"));
    }

    #[test]
    fn the_tabs_mark_the_section_being_shown() {
        let tabs = tabs(2, "characters");
        assert!(tabs[0].contains("href=\"/players/2\""));
        assert!(tabs[1].contains("class=\"current\" href=\"/players/2/characters\""));
        assert!(tabs[2].contains("<span>Shared storage</span>"));
    }
}
