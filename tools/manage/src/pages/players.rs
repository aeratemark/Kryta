//! Players: the account directory, and one account with everything that belongs to it.
//!
//! The hierarchy is the game's own: an account is the login identity, and it owns its characters
//! (the players), whose own pages carry their progress, position, inventory, titles and unlocks.
//! Account-wide data that this server does not record yet is named here and not pretended.

use shared::network::http::{Request, Response};

use crate::web::{components, http, layout};
use crate::view;
use crate::Manage;

/// The account directory, and the place a new account is made.
pub fn index(manage: &Manage) -> Response {
    let rows = match view::accounts(manage) {
        Ok(rows) => rows,
        Err(error) => {
            return crate::pages::error(
                500,
                "players",
                &format!("The account table could not be read: {error}"),
            );
        }
    };
    let active = rows.iter().filter(|row| row.account.active).count();
    let directory = rows
        .iter()
        .map(|row| directory_row(row, false))
        .collect::<String>();
    let directory = if directory.is_empty() {
        components::note("No account exists yet. Create the first one on this page.")
    } else {
        directory
    };

    let body = format!(
        "<p class=\"note\">Players is the entry point for player administration: an account is the\
         login identity, and it owns the characters that page lists. An inactive account starts no\
         later login but keeps its characters.</p>\n\
         <section class=\"metric-grid\">\
         {total}{active}{inactive}</section>\n\
         <section class=\"workspace\">\n\
         <aside class=\"panel directory\"><div class=\"directory-head\"><h2>Account directory</h2>\
         <p>Select an account to manage its login identity and characters.</p></div>\
         <div class=\"directory-list\">{directory}</div></aside>\n\
         {create}\n\
         </section>",
        total = components::metric(rows.len(), "accounts"),
        active = components::metric(active, "active for login"),
        inactive = components::metric(rows.len().saturating_sub(active), "inactive, retained"),
        create = create_form(),
    );
    layout::page("Players", "players", &body)
}

/// One account: its identity, its login state, and the players it owns.
pub fn account(manage: &Manage, id: i64) -> Response {
    let rows = match view::accounts(manage) {
        Ok(rows) => rows,
        Err(error) => {
            return crate::pages::error(
                500,
                "players",
                &format!("The account table could not be read: {error}"),
            );
        }
    };
    let Some(row) = rows.iter().find(|row| row.account.id == id) else {
        return crate::pages::error(404, "players", "No account has that id.");
    };
    let account = &row.account;
    let name = components::escape(&account.username);

    let tabs = vec![
        components::tab(Some(&format!("/players/{id}")), "Overview", true),
        components::tab(
            Some(&format!("/players/{id}/characters")),
            &format!("Characters ({})", row.characters),
            false,
        ),
        components::tab(None, "Shared storage", false),
        components::tab(None, "Account unlocks", false),
    ];

    let roster = match manage.characters_of_account(&account.user_id) {
        Some(characters) if characters.is_empty() => components::note(
            "No character exists on this account yet. Create one from its Characters section.",
        ),
        Some(characters) => {
            let names = view::map_names(manage);
            let rows = characters
                .iter()
                .map(|character| {
                    let row = view::CharacterRow::of(character, &names);
                    let href = format!("/players/{id}/characters/{}", row.uuid);
                    components::row(&[
                        format!("<a href=\"{href}\">{}</a>", components::escape(&row.name)),
                        row.level.to_string(),
                        components::escape(&row.campaign),
                        components::escape(&row.professions),
                    ])
                })
                .collect::<Vec<_>>();
            components::table(&["player", "level", "campaign", "professions"], &rows)
        }
        None => components::note(
            "No character database is configured in manage.json, so this account's roster cannot be read.",
        ),
    };

    let eula = if account.eula == 0 {
        "The client is asked to accept the EULA (no version recorded)"
    } else {
        "The client was answered with this accepted version"
    };
    let eula_action = if account.eula == 0 {
        "Record acceptance (version 26)"
    } else {
        "Ask the client again (clear the version)"
    };
    let active_action = if account.active {
        "Deactivate account"
    } else {
        "Activate account"
    };
    let next_active = !account.active;
    let id_text = id.to_string();

    let inspector = format!(
        "<section class=\"panel\"><div class=\"panel-head\"><div><h2>{name}</h2>\
         <p>Account #{id} - {badge}</p></div><span>{characters}</span></div>{tabs}\
         <form class=\"editor-form\" method=\"post\" action=\"/players/{id}/update\">\
         {login}{pair}\
         <div class=\"actions\">{save}</div></form>\
         <hr class=\"section-rule\"><div class=\"panel-head\"><div><h3>Login state</h3>\
         <p>{eula}. Deactivation affects the next authentication only; it does not end a session\
         already in progress.</p></div></div>\
         <div class=\"actions\">\
         <form method=\"post\" action=\"/players/{id}/eula\">{eula_button}</form>\
         <form method=\"post\" action=\"/players/{id}/active\">{active_hidden}{active_button}</form>\
         </div>\
         <hr class=\"section-rule\"><div class=\"panel-head\"><div><h3>Permanent removal</h3>\
         <p>Removes this login account only. Its character records are retained and can be removed\
         from the character pages when explicitly intended.</p></div></div>\
         <form method=\"post\" action=\"/players/{id}/delete\" onsubmit=\"return confirm('Remove {confirm}? Its characters are retained.')\">\
         {remove}</form></section>",
        name = name,
        id = id_text,
        badge = components::badge(account.active),
        characters = format!("{} character(s)", row.characters),
        tabs = components::section_nav(&tabs),
        login = components::field("Login name", &components::text_input("username", &account.username)),
        pair = format!(
            "<div class=\"field-grid\">{}{}</div>",
            components::field("Portal user id", &components::text_input("user_id", &account.user_id)),
            components::field("Portal token", &components::text_input("token", &account.token)),
        ),
        save = components::submit("Save account"),
        eula = eula,
        eula_button = components::submit_class(eula_action, "secondary"),
        active_hidden = components::hidden("active", if next_active { "true" } else { "false" }),
        active_button = components::submit_class(active_action, "secondary"),
        confirm = name,
        remove = components::submit_class("Remove account", "danger"),
    );

    let players = format!(
        "<section class=\"panel\"><div class=\"panel-head\"><div><h2>Players</h2>\
         <p>The characters this account owns. A character's page owns its progress, position,\
         inventory, titles and unlocks.</p></div>\
         <a class=\"button\" href=\"/players/{id}/characters\">Open characters</a></div>{roster}</section>",
        id = id_text,
    );

    let body = format!(
        "{crumbs}\n{inspector}\n{players}\n\
         <p class=\"note\">Shared storage and account-wide unlocks belong to an account in the game;\
         this server does not record either yet, so there is nothing to show or edit here.</p>",
        crumbs = components::crumbs(&[("/players", "Players"), ("", &name)]),
    );
    layout::page("Players", "players", &body)
}

/// `POST /players/create`: record a new login identity.
pub fn create(manage: &Manage, request: &Request) -> Response {
    let form = http::form_fields(&request.body_text());
    let username = http::value(&form, "username").trim();
    if username.is_empty() {
        return crate::pages::error(400, "players", "An account needs a login name.");
    }
    let user_id = match http::value(&form, "user_id").trim() {
        "" => generated_id(),
        stated => stated.to_string(),
    };
    let token = match http::value(&form, "token").trim() {
        "" => generated_id(),
        stated => stated.to_string(),
    };
    match manage.accounts().create(username, &user_id, &token) {
        Ok(true) => {
            manage.say(format!("{username}: account created"));
            let id = manage
                .list_accounts()
                .ok()
                .and_then(|accounts| accounts.into_iter().find(|account| account.username == username))
                .map(|account| account.id);
            match id {
                Some(id) => http::redirect(&format!("/players/{id}")),
                None => http::redirect("/players"),
            }
        }
        Ok(false) => crate::pages::error(
            409,
            "players",
            &format!("An account is already called {username}."),
        ),
        Err(error) => crate::pages::error(
            500,
            "players",
            &format!("The account table said: {error}"),
        ),
    }
}

/// `POST /players/{id}/update`: change the three values the portal login checks.
pub fn update(manage: &Manage, request: &Request, id: i64) -> Response {
    let form = http::form_fields(&request.body_text());
    let username = http::value(&form, "username").trim();
    if username.is_empty() {
        return crate::pages::error(400, "players", "An account login name is required.");
    }
    let user_id = http::value(&form, "user_id");
    let token = http::value(&form, "token");
    match manage.accounts().update(id, username, user_id, token) {
        Ok(true) => {
            manage.say(format!("account {id}: updated as {username}"));
            http::redirect(&format!("/players/{id}"))
        }
        Ok(false) => crate::pages::error(404, "players", &format!("No account has id {id}.")),
        Err(error) => crate::pages::error(
            500,
            "players",
            &format!("The account table said: {error}"),
        ),
    }
}

/// `POST /players/{id}/active`: admit or refuse future logins, keeping the row and its characters.
pub fn set_active(manage: &Manage, request: &Request, id: i64) -> Response {
    let form = http::form_fields(&request.body_text());
    let active = http::value(&form, "active") == "true";
    match manage.accounts().set_active(id, active) {
        Ok(true) => {
            let state = if active { "activated" } else { "deactivated" };
            manage.say(format!("account {id}: {state}"));
            http::redirect(&format!("/players/{id}"))
        }
        Ok(false) => crate::pages::error(404, "players", &format!("No account has id {id}.")),
        Err(error) => crate::pages::error(
            500,
            "players",
            &format!("The account table said: {error}"),
        ),
    }
}

/// `POST /players/{id}/eula`: record that the client accepted the EULA, or make it ask again.
///
/// The column is the version the login answer carries back: a recorded version makes the client
/// stop asking, and zero makes it ask again (`account.eula`).
pub fn set_eula(manage: &Manage, request: &Request, id: i64) -> Response {
    let _ = request;
    let Ok(accounts) = manage.list_accounts() else {
        return crate::pages::error(500, "players", "The account table could not be read.");
    };
    let Some(account) = accounts.iter().find(|account| account.id == id) else {
        return crate::pages::error(404, "players", &format!("No account has id {id}."));
    };
    let version: i64 = if account.eula == 0 { 26 } else { 0 };
    match manage.accounts().set_eula(&account.username, version) {
        Ok(true) => {
            manage.say(format!(
                "account {id}: the EULA version is now {version} ({} accepted)",
                account.username
            ));
            http::redirect(&format!("/players/{id}"))
        }
        Ok(false) => crate::pages::error(404, "players", "The account disappeared while it was read."),
        Err(error) => crate::pages::error(
            500,
            "players",
            &format!("The account table said: {error}"),
        ),
    }
}

/// `POST /players/{id}/delete`: remove the login identity only; character records are kept.
pub fn delete(manage: &Manage, request: &Request, id: i64) -> Response {
    let _ = request;
    let Ok(accounts) = manage.list_accounts() else {
        return crate::pages::error(500, "players", "The account table could not be read.");
    };
    let Some(account) = accounts.iter().find(|account| account.id == id) else {
        return crate::pages::error(404, "players", &format!("No account has id {id}."));
    };
    match manage.accounts().delete(&account.username) {
        Ok(true) => {
            manage.say(format!(
                "{}: account removed (its characters are untouched)",
                account.username
            ));
            http::redirect("/players")
        }
        Ok(false) => crate::pages::error(404, "players", "The account is already gone."),
        Err(error) => crate::pages::error(
            500,
            "players",
            &format!("The account table said: {error}"),
        ),
    }
}

/// One compact directory row; edits belong to the selected account's page, not to every row.
fn directory_row(row: &view::AccountRow, selected: bool) -> String {
    let account = &row.account;
    let class = if selected {
        "directory-row selected"
    } else {
        "directory-row"
    };
    format!(
        "<a class=\"{class}\" href=\"/players/{id}\"><span><strong>{name}</strong>\
         <small>#{id} - {characters} character(s)</small></span>{badge}</a>",
        id = account.id,
        name = components::escape(&account.username),
        characters = row.characters,
        badge = components::badge(account.active),
    )
}

/// The aside a directory has no selection for: a new account's form.
fn create_form() -> String {
    format!(
        "<section class=\"panel\"><div class=\"panel-head\"><div><h2>Create account</h2>\
         <p>Create a login identity, then open it to reach its characters.</p></div></div>\
         <form class=\"editor-form\" method=\"post\" action=\"/players/create\">\
         {login}{pair}\
         <div class=\"actions\">{button}</div></form>\
         <p class=\"note\">The portal issues the identity pair the client presents. Leave either\
         value blank only when this local manager should generate it.</p></section>",
        login = components::field(
            "Login name",
            &components::text_input_placeholder("username", "name@example.com")
        ),
        pair = format!(
            "<div class=\"field-grid\">{}{}</div>",
            components::field(
                "Portal user id",
                &components::text_input_placeholder("user_id", "generated when blank")
            ),
            components::field(
                "Portal token",
                &components::text_input_placeholder("token", "generated when blank")
            ),
        ),
        button = components::submit("Create account"),
    )
}

/// A portal identifier for a field the form left blank: the shape the account table already holds.
fn generated_id() -> String {
    format!(
        "00000000-0000-0000-0000-{:012}",
        crate::random_u64() % 1_000_000_000_000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_portal_id_keeps_the_tables_shape() {
        let id = generated_id();
        assert!(id.starts_with("00000000-0000-0000-0000-"));
        assert_eq!(id.len(), 36);
    }
}
