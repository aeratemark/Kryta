//! The pages: HTML written here, no template engine and no asset pipeline.
//!
//! A page is a function from the tool's state to an HTML body. Nothing is fetched by the browser
//! beyond the page itself, so a form is a plain `POST` and an answer is another page: the whole
//! interface works on a machine with no JavaScript at all.

use shared::network::http::Response;

use crate::Manage;

/// The shell every page is written into.
fn page(title: &str, body: &str) -> Response {
    let html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <title>Kryta - {title}</title>\n\
         <style>\n\
         body{{font-family:system-ui,sans-serif;margin:2rem;max-width:60rem}}\n\
         nav a{{margin-right:1rem}}\n\
         table{{border-collapse:collapse;width:100%;margin:1rem 0}}\n\
         th,td{{text-align:left;padding:.35rem .6rem;border-bottom:1px solid #ddd;font-size:.95rem}}\n\
         th{{background:#f4f4f4}}\n\
         code{{background:#f4f4f4;padding:.1rem .3rem}}\n\
         .note{{color:#555;font-size:.9rem}}\n\
         input[type=text]{{width:16rem}}\n\
         </style></head><body>\n\
         <nav><a href=\"/\">overview</a><a href=\"/accounts\">accounts</a>\
         <a href=\"/characters\">characters</a><a href=\"/worlds\">worlds</a>\
         <a href=\"/content\">content</a><a href=\"/schema\">schema</a></nav>\n\
         <h1>{title}</h1>\n{body}\n</body></html>\n"
    );
    Response::html(200, html)
}

/// The overview: what this tool holds, and what it knows about.
pub fn dashboard(manage: &Manage) -> Response {
    let accounts = match manage.list_accounts() {
        Ok(accounts) => accounts,
        Err(error) => {
            return Response::html(
                500,
                message(&format!("The account table could not be read: {error}")),
            );
        }
    };
    let accepted = accounts.iter().filter(|account| account.eula != 0).count();
    let characters = manage.list_characters();
    let characters_state = match &characters {
        Some(characters) => format!(
            "{} row(s) - <a href=\"/characters\">open</a>",
            characters.len()
        ),
        None => "no character database configured".to_string(),
    };
    let worlds = manage.list_worlds();
    let worlds_state = match &worlds {
        Some(worlds) => format!("{} row(s) - <a href=\"/worlds\">open</a>", worlds.len()),
        None => "the login database could not be read".to_string(),
    };
    let content = manage.list_maps();
    let content_state = match &content {
        Some((maps, spawns)) => format!(
            "{} map(s), {} spawn point(s) - <a href=\"/content\">open</a>",
            maps.len(),
            spawns.len()
        ),
        None => "no world database configured".to_string(),
    };

    let body = format!(
        "<p>The administration tool. It writes the same rows the servers read, through the same \
         code: nothing here is a second implementation of the data.</p>\n\
         <table>\n\
         <tr><th>what</th><th>where</th><th>state</th></tr>\n\
         <tr><td>accounts</td><td><code>kryta_login.account</code></td><td>{} row(s), {accepted} with an EULA accepted - <a href=\"/accounts\">open</a></td></tr>\n\
         <tr><td>characters</td><td><code>kryta_characters.characters</code></td><td>{characters_state}</td></tr>\n\
         <tr><td>worlds</td><td><code>realmlist</code> + <code>data/realms.json</code></td><td>{worlds_state}</td></tr>\n\
         <tr><td>content</td><td>the world database's rows (<code>map</code>, <code>map_spawn</code>)</td><td>{content_state}</td></tr>\n\
         <tr><td>schema</td><td>each database's <code>updates</code> record</td><td><a href=\"/schema\">open</a></td></tr>\n\
         </table>\n\
         <p class=\"note\">The tool serves this machine only, by default. A token in \
         <code>manage.json</code> makes it require one.</p>",
        accounts.len()
    );
    page("overview", &body)
}

/// The characters: every row the login lists, read through the noun's own repository.
pub fn characters(manage: &Manage) -> Response {
    let Some(characters) = manage.list_characters() else {
        return Response::html(
            503,
            message("No character database is configured in manage.json, so there is nothing to list."),
        );
    };

    let mut rows = String::new();
    for character in &characters {
        let items = manage
            .list_items_of(&game::characters::uuid_text(&character.uuid))
            .map_or_else(|| "<em>?</em>".to_string(), |items| items.len().to_string());
        rows.push_str(&format!(
            "<tr><td><code>{name}</code></td><td><code>{uuid}</code></td>\
             <td>{campaign}</td><td>{level}</td><td>{outpost}</td><td>{items}</td><td>{pvp}</td></tr>\n",
            name = escape(&character.name),
            uuid = escape(&game::characters::uuid_text(&character.uuid)),
            campaign = character.settings.campaign,
            level = character.settings.level,
            outpost = character.settings.last_outpost,
            items = items,
            pvp = if character.settings.is_pvp { "yes" } else { "no" },
        ));
    }

    let body = format!(
        "<p class=\"note\">Every row of the character table, as the login's own list reads them: a \
         name, the id the client knows it by, its campaign, level, last outpost, how many items it \
         holds and its PvP flag.</p>\n\
         <table>\n<tr><th>name</th><th>uuid</th><th>campaign</th><th>level</th><th>last outpost</th>\
         <th>items</th><th>PvP</th></tr>\n{rows}</table>\n\
         <p class=\"note\">{count} row(s). A character is written by the creation confirm, not here: \
         this page shows what the servers will read.</p>",
        count = characters.len()
    );
    page("characters", &body)
}

/// The worlds: the login database's `realmlist`, merged with `data/realms.json`.
pub fn worlds(manage: &Manage) -> Response {
    let Some(worlds) = manage.list_worlds() else {
        return Response::html(
            500,
            message("The login database could not be read, so the world list is not available."),
        );
    };

    let mut rows = String::new();
    for world in &worlds {
        rows.push_str(&format!(
            "<tr><td>{id}</td><td>{name}</td><td><code>{address}:{port}</code></td>\
             <td><code>{local}:{port}</code></td></tr>\n",
            id = world.id,
            name = escape(&world.name),
            address = escape(&world.address_text),
            port = world.port,
            local = world.local_address,
        ));
    }

    let body = format!(
        "<p class=\"note\">The worlds a login may send a player to, after `data/realms.json` is \
         merged over the `realmlist` table. The two programs read the same list; the file row wins \
         when both name a world.</p>\n\
         <table>\n<tr><th>id</th><th>name</th><th>public address</th><th>local address</th></tr>\n\
         {rows}</table>\n\
         <p class=\"note\">{count} world(s).</p>",
        count = worlds.len()
    );
    page("worlds", &body)
}

/// The content: the world's maps and their spawn points, and the item templates, read through the
/// nouns' own repositories.
pub fn content(manage: &Manage) -> Response {
    let Some((maps, spawns)) = manage.list_maps() else {
        return Response::html(
            503,
            message("No world database is configured in manage.json, so there is no content to show."),
        );
    };

    let mut rows = String::new();
    for map in &maps {
        let points: Vec<String> = spawns
            .iter()
            .filter(|spawn| spawn.map_id == map.id)
            .map(|spawn| {
                format!(
                    "#{index} x {x:.2} y {y:.2} plane {plane} \
                     <form method=\"post\" action=\"/content/spawn/remove\" style=\"display:inline\">\
                     <input type=\"hidden\" name=\"map_id\" value=\"{map_id}\">\
                     <input type=\"hidden\" name=\"index\" value=\"{index}\">\
                     <button type=\"submit\">remove</button></form>",
                    index = spawn.index,
                    x = spawn.x,
                    y = spawn.y,
                    plane = spawn.plane,
                    map_id = map.id,
                )
            })
            .collect();
        rows.push_str(&format!(
            "<tr><td>{id}</td><td>{kind}</td><td><code>{file_id}</code></td>\
             <td>{name}</td><td>{points}<br><form method=\"post\" action=\"/content/spawn\">\
             <input type=\"hidden\" name=\"map_id\" value=\"{id}\">\
             x <input type=\"text\" name=\"x\" size=\"8\"> \
             y <input type=\"text\" name=\"y\" size=\"8\"> \
             plane <input type=\"text\" name=\"plane\" size=\"3\" value=\"0\"> \
             <button type=\"submit\">add a spawn point</button></form></td></tr>\n",
            id = map.id,
            kind = map.kind,
            file_id = map.file_id,
            name = escape(&map.name),
            points = if points.is_empty() {
                "<em>none</em>".to_string()
            } else {
                points.join("<br>")
            },
        ));
    }

    let items = manage.list_items().unwrap_or_default();
    let mut item_rows = String::new();
    for item in &items {
        item_rows.push_str(&format!(
            "<tr><td><code>{file_id}</code></td><td>{item_type}</td><td>{dye_tint}/{dye_colors}</td>\
             <td>{model_id}</td><td>{value}</td><td>{profession}</td><td>{quantity}</td>\
             <td>{name}</td></tr>\n",
            file_id = item.file_id,
            item_type = item.item_type,
            dye_tint = item.dye_tint,
            dye_colors = item.dye_colors,
            model_id = item.model_id,
            value = item.value,
            profession = item.profession,
            quantity = item.quantity,
            name = item
                .name
                .iter()
                .map(|unit| unit.to_string())
                .collect::<Vec<_>>()
                .join(" "),
        ));
    }

    let body = format!(
        "<p class=\"note\">The maps a world can put a client in, and where a character may stand on \
         each - the rows the load reads (<code>0x0196</code>), straight from \
         <code>kryta_world.map</code> and <code>kryta_world.map_spawn</code>. A spawn point added \
         here is where every entry may place a character, scattered per R13; one removed is one the \
         load will not pick again.</p>\n\
         <table>\n<tr><th>id</th><th>kind</th><th>file id</th><th>name</th><th>spawn points</th></tr>\n\
         {rows}</table>\n\
         <p class=\"note\">{maps} map(s), {spawns} spawn point(s). Kinds: 0 outpost, 1 explorable, \
         11 character creation.</p>\n\
         <h2>item templates</h2>\n\
         <p class=\"note\">What an item is, once: the rows the creation record lists seed at start-up. \
         A character's own items point at these.</p>\n\
         <table>\n<tr><th>file id</th><th>type</th><th>dyes</th><th>model</th><th>value</th>\
         <th>profession</th><th>stack</th><th>name units</th></tr>\n{item_rows}</table>\n\
         <p class=\"note\">{items} template(s).</p>",
        maps = maps.len(),
        spawns = spawns.len(),
        items = items.len()
    );
    page("content", &body)
}

/// The accounts: the rows the login reads, and the three things an operator changes about them.
pub fn accounts(manage: &Manage) -> Response {
    let accounts = match manage.list_accounts() {
        Ok(accounts) => accounts,
        Err(error) => {
            return Response::html(
                500,
                message(&format!("The account table could not be read: {error}")),
            );
        }
    };

    let mut rows = String::new();
    for account in &accounts {
        let eula = if account.eula == 0 {
            "<em>none</em>".to_string()
        } else {
            format!("version {}", account.eula)
        };
        rows.push_str(&format!(
            "<tr><td>{id}</td><td><code>{name}</code></td><td><code>{user_id}</code></td>\
             <td><code>{token}</code></td><td>{eula}</td>\
             <td><form method=\"post\" action=\"/accounts/eula\" style=\"display:inline\">\
             <input type=\"hidden\" name=\"username\" value=\"{name}\">\
             <input type=\"text\" name=\"version\" value=\"{next}\" size=\"4\">\
             <button type=\"submit\">set EULA</button></form></td>\
             <td><form method=\"post\" action=\"/accounts/delete\" style=\"display:inline\" \
             onsubmit=\"return confirm('Remove {name}? Its characters are left alone.')\">\
             <input type=\"hidden\" name=\"username\" value=\"{name}\">\
             <button type=\"submit\">remove</button></form></td></tr>\n",
            id = account.id,
            name = escape(&account.username),
            user_id = escape(&account.user_id),
            token = escape(&account.token),
            eula = eula,
            next = if account.eula == 0 { 26 } else { account.eula },
        ));
    }

    let body = format!(
        "<p class=\"note\">The EULA version the client accepted is a column: a non-zero value stops \
         the client asking, and 0 makes it ask again.</p>\n\
         <table>\n<tr><th>id</th><th>username</th><th>user id</th><th>token</th><th>EULA</th>\
         <th>set the EULA</th><th></th></tr>\n{rows}</table>\n\
         <h2>create an account</h2>\n\
         <form method=\"post\" action=\"/accounts/create\">\n\
         <label>login name <input type=\"text\" name=\"username\" placeholder=\"name@example.com\"></label>\n\
         <label>user id <input type=\"text\" name=\"user_id\" placeholder=\"issued by the portal\"></label>\n\
         <label>token <input type=\"text\" name=\"token\" placeholder=\"issued by the portal\"></label>\n\
         <button type=\"submit\">create</button></form>\n\
         <p class=\"note\">An identifier left empty is generated. The portal hands the client these \
         two values, and a login is checked against them.</p>"
    );
    page("accounts", &body)
}

/// The schema: each database's own record of the files it has applied, against the files on disk.
pub fn schema(manage: &Manage) -> Response {    let mut sections = String::new();
    for (label, files) in manage.schema_page() {
        let mut rows = String::new();
        for file in &files {
            rows.push_str(&format!(
                "<tr><td><code>{name}</code></td><td>{state}</td></tr>\n",
                name = escape(&file.name),
                state = file.state
            ));
        }
        sections.push_str(&format!(
            "<h2>{label}</h2>\n<table>\n<tr><th>file</th><th>state</th></tr>\n{rows}</table>\n\
             <p class=\"note\">{} file(s).</p>\n",
            files.len()
        ));
    }

    let body = format!(
        "<p class=\"note\">A database records each schema file it has had in its own \
         <code>updates</code> table. A file listed as <em>new</em> has not been applied yet; \
         <em>changed</em> means the file was edited after it was applied, which the updater refuses \
         to apply again until an operator resolves it; <em>recorded, not on disk</em> means the \
         database holds a record for a file this checkout does not carry.</p>\n{sections}"
    );
    page("schema", &body)
}

/// A short page saying one thing.
pub fn message(text: &str) -> String {
    format!(
        "<p>{}</p><p><a href=\"/accounts\">back to accounts</a></p>",
        escape(text)
    )
}

/// The 401 page.
pub fn unauthorised() -> String {
    "<p>This tool wants its token: add <code>?token=&lt;value&gt;</code> to the address once.</p>"
        .to_string()
}

/// The 404 page.
pub fn missing(path: &str) -> String {
    message(&format!("No page is at {path}."))
}

/// A redirect: a form's answer is another page, and the browser goes there.
pub fn redirect(to: &str) -> Response {
    Response::new(303, "text/html; charset=utf-8", format!("<p>moved to <a href=\"{to}\">{to}</a></p>").into_bytes())
}

/// Escape text for HTML: a name or a token is data, never markup.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}
