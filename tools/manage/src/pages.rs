//! The pages: HTML written here, no template engine and no asset pipeline.
//!
//! A page is a function from the tool's state to an HTML body. Nothing is fetched by the browser
//! beyond the page itself, so a form is a plain `POST` and an answer is another page: the whole
//! interface works on a machine with no JavaScript at all.

use common::configuration::{Config, Policy};
use shared::network::http::Response;

use crate::Manage;

/// The shell every page is written into. It is an operator workspace: information stays compact,
/// the active work area is explicit, and every later feature inherits the same frame.
fn page(title: &str, body: &str) -> Response {
    let html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Kryta Control - {title}</title>\n\
         <style>\n\
         :root{{color-scheme:dark;--void:#0a0d12;--rail:#10151d;--surface:#151b25;--surface-raised:#1b2330;--surface-hover:#222c3b;--line:#2b3545;--text:#eef2f7;--muted:#9ca9b9;--accent:#e7a55e;--accent-strong:#ffc984;--accent-ink:#2d1c0c;--good:#66c6a4;--warning:#e9b765;--danger:#e07a82;--danger-ink:#331317;--focus:#8aa9ff;--shadow:0 16px 36px rgba(0,0,0,.24)}}\n\
         *{{box-sizing:border-box}}\n\
         body{{margin:0;background:var(--void);color:var(--text);font-family:Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,\"Segoe UI\",sans-serif;line-height:1.45}}\n\
         .shell{{display:grid;grid-template-columns:15.25rem minmax(0,1fr);min-height:100vh}}\n\
         .rail{{padding:1.25rem .85rem;background:var(--rail);border-right:1px solid var(--line);color:var(--text);display:flex;flex-direction:column;gap:1.8rem}}\n\
         .brand{{color:inherit;text-decoration:none;display:flex;align-items:center;gap:.7rem;padding:.5rem .55rem}}\n\
         .brand-mark{{display:grid;place-items:center;width:2rem;height:2rem;border:1px solid rgba(255,201,132,.5);border-radius:.45rem;background:linear-gradient(135deg,#f6c077,#c47536);color:var(--accent-ink);font-size:1rem;font-weight:900;box-shadow:0 6px 16px rgba(231,165,94,.12)}}\n\
         .brand strong{{display:block;font-size:.95rem;letter-spacing:.03em}}\n\
         .brand small{{display:block;color:var(--muted);font-size:.66rem;letter-spacing:.13em;text-transform:uppercase}}\n\
         .nav-group{{display:grid;gap:.2rem;margin-bottom:1.2rem}}\n\
         .rail-label{{margin:0 .55rem .35rem;color:#738196;font-size:.64rem;font-weight:750;letter-spacing:.14em;text-transform:uppercase}}\n\
         nav{{display:grid;gap:.18rem}}\n\
         nav a{{color:#b9c3d1;text-decoration:none;padding:.52rem .65rem;border:1px solid transparent;border-radius:.42rem;font-size:.86rem;font-weight:650;transition:background .12s,color .12s,border-color .12s}}\n\
         nav a:hover{{background:var(--surface);border-color:var(--line);color:var(--text)}}\n\
         nav a.active{{background:rgba(231,165,94,.14);border-color:rgba(231,165,94,.28);color:var(--accent-strong)}}\n\
         .rail-foot{{margin-top:auto;padding:.8rem .55rem;color:#7f8da0;font-size:.72rem;border-top:1px solid var(--line)}}\n\
         main{{width:100%;max-width:106rem;padding:1.5rem clamp(1.5rem,3vw,3rem) 3rem;margin:0 auto}}\n\
         .page-head{{display:flex;align-items:flex-end;justify-content:space-between;gap:1rem;padding:0 0 1.25rem;border-bottom:1px solid var(--line);margin-bottom:1.35rem}}\n\
         .eyebrow{{margin:0 0:.35rem;color:var(--accent);font-size:.65rem;font-weight:800;letter-spacing:.14em;text-transform:uppercase}}\n\
         h1{{margin:0;font-size:1.72rem;line-height:1.1;letter-spacing:-.028em;color:var(--text)}}\n\
         h2{{margin:0;font-size:1rem;letter-spacing:-.01em;color:var(--text)}}\n\
         h3{{margin:0;font-size:.84rem;color:var(--muted);font-weight:700;text-transform:uppercase;letter-spacing:.08em}}\n\
         p{{max-width:78ch;margin:.45rem 0;color:var(--muted)}}\n\
         .local-mark{{display:inline-flex;align-items:center;gap:.4rem;color:var(--muted);font-size:.75rem;white-space:nowrap}}\n\
         .local-mark::before{{content:\"\";width:.45rem;height:.45rem;border-radius:50%;background:var(--good);box-shadow:0 0 0 3px rgba(102,198,164,.12)}}\n\
         .panel{{background:var(--surface);border:1px solid var(--line);border-radius:.65rem;padding:1rem;box-shadow:var(--shadow)}}\n\
         .panel + .panel{{margin-top:1rem}}\n\
         .panel-head{{display:flex;align-items:flex-start;justify-content:space-between;gap:1rem;margin-bottom:.85rem}}\n\
         .metric-grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(10rem,1fr));gap:.75rem;margin-bottom:1rem}}\n\
         .metric{{background:var(--surface);border:1px solid var(--line);border-radius:.6rem;padding:.85rem 1rem}}\n\
         .metric strong{{display:block;color:var(--text);font-size:1.45rem;line-height:1.1}}\n\
         .metric span{{display:block;color:var(--muted);font-size:.74rem;margin-top:.25rem}}\n\
         .workspace{{display:grid;grid-template-columns:minmax(17rem,.8fr) minmax(22rem,1.2fr);gap:1rem;align-items:start}}\n\
         .directory{{padding:0;overflow:hidden}}\n\
         .directory-head{{padding:1rem;border-bottom:1px solid var(--line)}}\n\
         .directory-list{{display:grid}}\n\
         .directory-row{{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:.75rem;align-items:center;color:var(--text);text-decoration:none;padding:.75rem 1rem;border-bottom:1px solid var(--line)}}\n\
         .directory-row:last-child{{border-bottom:0}}\n\
         .directory-row:hover{{background:var(--surface-hover)}}\n\
         .directory-row.selected{{background:rgba(138,169,255,.12);box-shadow:inset 3px 0 var(--focus)}}\n\
         .directory-row strong{{display:block;font-size:.9rem;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}\n\
         .directory-row small{{display:block;color:var(--muted);font-size:.72rem;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;margin-top:.15rem}}\n\
         .badge{{display:inline-flex;align-items:center;border-radius:99px;padding:.16rem .45rem;font-size:.66rem;font-weight:800;letter-spacing:.04em;text-transform:uppercase}}\n\
         .badge.active{{background:rgba(102,198,164,.14);color:var(--good)}}\n\
         .badge.inactive{{background:rgba(224,122,130,.14);color:var(--danger)}}\n\
         .editor-form{{display:grid;gap:.85rem}}\n\
         .field-grid{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:.7rem}}\n\
         label{{display:grid;gap:.32rem;color:var(--muted);font-size:.72rem;font-weight:700}}\n\
         input[type=text],textarea{{width:100%;padding:.53rem .6rem;border:1px solid var(--line);border-radius:.38rem;background:#0d121a;color:var(--text);font:inherit;font-size:.86rem;outline:none}}\n\
         input[type=text]:focus,textarea:focus{{border-color:var(--focus);box-shadow:0 0 0 3px rgba(138,169,255,.13)}}\n\
         textarea{{min-height:6rem;resize:vertical}}\n\
         button,.button{{display:inline-flex;align-items:center;justify-content:center;padding:.48rem .7rem;border:1px solid transparent;border-radius:.38rem;background:var(--accent);color:var(--accent-ink);font:inherit;font-size:.78rem;font-weight:800;cursor:pointer;text-decoration:none}}\n\
         button:hover,.button:hover{{background:var(--accent-strong)}}\n\
         button.secondary,.button.secondary{{background:transparent;border-color:var(--line);color:var(--text)}}\n\
         button.secondary:hover,.button.secondary:hover{{background:var(--surface-hover)}}\n\
         button.danger{{background:var(--danger);color:var(--danger-ink)}}\n\
         .actions{{display:flex;gap:.5rem;flex-wrap:wrap;margin-top:.15rem}}\n\
         .actions form{{margin:0}}\n\
         .section-rule{{border:0;border-top:1px solid var(--line);margin:1rem 0}}\n\
         table{{border-collapse:separate;border-spacing:0;width:100%;margin:0;background:var(--surface);border:1px solid var(--line);border-radius:.6rem;overflow:hidden}}\n\
         th,td{{text-align:left;padding:.62rem .75rem;border-bottom:1px solid var(--line);font-size:.8rem;vertical-align:top}}\n\
         tr:last-child td{{border-bottom:0}}\n\
         th{{background:var(--surface-raised);color:var(--muted);font-size:.64rem;letter-spacing:.08em;text-transform:uppercase}}\n\
         code,pre{{font-family:\"Cascadia Code\",\"SFMono-Regular\",Consolas,monospace}}\n\
         code{{display:inline-block;padding:.1rem .28rem;background:#0d121a;border:1px solid var(--line);border-radius:.25rem;color:#c8d7e9;font-size:.86em;overflow-wrap:anywhere}}\n\
         pre{{padding:.85rem 1rem;max-width:100%;overflow:auto;background:#0c1118;color:#c8d7e9;border-radius:.5rem;border:1px solid var(--line)}}\n\
         .note{{padding:.7rem .8rem;background:rgba(138,169,255,.08);border:1px solid rgba(138,169,255,.19);border-radius:.45rem;color:#b8c6d8;font-size:.8rem}}\n\
         a{{color:#a9c0ff}}\n\
         @media (max-width:52rem){{.shell{{display:block}}.rail{{padding:.85rem;gap:.8rem;border-bottom:1px solid var(--line);border-right:0}}.rail nav{{display:flex;overflow-x:auto;padding-bottom:.15rem}}.nav-group{{display:contents}}.rail-label,.rail-foot{{display:none}}nav a{{white-space:nowrap}}main{{padding:1.1rem}}.page-head{{align-items:flex-start;flex-direction:column}}.workspace{{grid-template-columns:1fr}}.field-grid{{grid-template-columns:1fr}}table{{display:block;overflow-x:auto}}}}\n\
         </style></head><body><div class=\"shell\">\n\
         <aside class=\"rail\"><a class=\"brand\" href=\"/\"><span class=\"brand-mark\">K</span><span><strong>Kryta Control</strong><small>Configuration Manager</small></span></a>\n\
         <nav aria-label=\"Kryta Control\">{}</nav>\n\
         <p class=\"rail-foot\">Local operator workspace<br>Server ownership stays with its services.</p></aside>\n\
         <main><header class=\"page-head\"><div><p class=\"eyebrow\">Kryta Control</p><h1>{title}</h1></div><span class=\"local-mark\">local operator surface</span></header>\n{body}\n</main></div></body></html>\n",
        navigation(title)
    );
    Response::html(200, html)
}

/// The Configuration Manager's stable top-level destinations. The accounts editor is a working
/// database-backed management surface, so it belongs beside the roadmap destinations rather than as
/// an undiscoverable legacy URL. The pending destinations remain visible without masquerading as done.
fn navigation(title: &str) -> String {
    const GROUPS: [(&str, &[(&str, &str, &str)]); 3] = [
        (
            "workspace",
            &[
                ("/", "overview", "overview"),
                ("/services", "services", "services"),
                ("/console", "console", "console"),
                ("/logs", "logs", "logs"),
            ],
        ),
        ("players", &[("/accounts", "accounts", "accounts")]),
        (
            "server data",
            &[
                ("/configuration", "configuration", "configuration"),
                ("/assets", "assets", "assets"),
            ],
        ),
    ];
    GROUPS
        .iter()
        .map(|(label, items)| {
            let links = items
                .iter()
                .map(|(path, name, active)| {
                    let class = if *active == title { " active" } else { "" };
                    format!("<a class=\"nav-link{class}\" href=\"{path}\">{name}</a>")
                })
                .collect::<String>();
            format!("<div class=\"nav-group\"><p class=\"rail-label\">{label}</p>{links}</div>")
        })
        .collect()
}

/// The services destination is a client of the supervisor's existing lifecycle channel.
pub fn services(manage: &Manage, action: Option<(&str, &str, Result<String, String>)>) -> Response {
    let action_result = match action {
        Some((_verb, service, Ok(answer))) => format!(
            "<p class=\"note\">Supervisor: <strong>{}</strong> <code>{}</code>.</p>",
            escape(&answer),
            escape(service)
        ),
        Some((verb, service, Err(error))) => format!(
            "<p class=\"note\">Supervisor could not {verb} <code>{}</code>: {}</p>",
            escape(service),
            escape(&error)
        ),
        None => String::new(),
    };
    let status = manage.supervisor_request("status");
    let state = match &status {
        Ok(answer) => format!("<pre>{}</pre>{}", escape(answer), service_actions(answer)),
        Err(error) => format!(
            "<p class=\"note\">The running supervisor did not answer: {}</p>\n\
             <p>Start the stack with <code>cargo run -p kryta-supervisor</code>, then reload this page. \
             The manager does not launch a second supervisor.</p>",
            escape(error)
        ),
    };
    page(
        "services",
        &format!(
            "<p>Service state and lifecycle actions come from the supervisor's existing loopback \
             channel. The manager asks; the supervisor remains the only owner of its processes.</p>\n\
             {action_result}<h2>Running stack</h2>{state}"
        ),
    )
}

/// One restart and stop control for every service the supervisor currently reports. The status response
/// is the supervisor's data, not a second service inventory maintained by the manager.
fn service_actions(status: &str) -> String {
    let controls = status
        .lines()
        .filter_map(|line| line.split_once(':').map(|(name, _)| name.trim()))
        .filter(|name| !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')))
        .map(|name| {
            let name = escape(name);
            format!(
                "<tr><td><code>{name}</code></td><td><div class=\"actions\">\
                 <form method=\"post\" action=\"/services/restart\"><input type=\"hidden\" name=\"service\" value=\"{name}\"><button>Restart</button></form>\
                 <form method=\"post\" action=\"/services/stop\"><input type=\"hidden\" name=\"service\" value=\"{name}\"><button class=\"danger\">Stop</button></form>\
                 </div></td></tr>"
            )
        })
        .collect::<Vec<_>>();
    if controls.is_empty() {
        return String::new();
    }
    format!(
        "<h2>Actions</h2><table><tr><th>service</th><th>supervisor action</th></tr>{}</table>",
        controls.join("\n")
    )
}

/// The configuration destination reserves the program-settings workflow (roadmap step 5).
pub fn configuration() -> Response {
    page(
        "configuration",
        "<p>Configuration editing is not implemented yet. Step 5 will use the programs' existing \
         JSON reader and validation rules to read, validate, save, and report whether a change needs \
         a restart. This page does not edit a file.</p>",
    )
}

/// The assets destination reserves generic JSON asset management (roadmap step 6).
pub fn assets() -> Response {
    page(
        "assets",
        "<p>Generic JSON asset browsing, editing, validation, and saving are not implemented yet. \
         Step 6 will add that workflow without introducing specialized game-asset editors. This page \
         does not change game data.</p>",
    )
}

/// The browser console is an adapter for the existing API and registry, never its own dispatcher.
pub fn console(
    manage: &Manage,
    run: Option<(
        &str,
        std::collections::BTreeMap<String, String>,
        Result<String, String>,
    )>,
) -> Response {
    let result = match run {
        Some((operation, _arguments, Ok(answer))) => format!(
            "<h2>Result: <code>{}</code></h2><pre>{}</pre>",
            escape(operation),
            escape(&answer)
        ),
        Some((operation, _arguments, Err(error))) => format!(
            "<h2>Refused: <code>{}</code></h2><p class=\"note\">{}</p>",
            escape(operation),
            escape(&error)
        ),
        None => String::new(),
    };
    let operations =
        match manage.world_operation("world.operations", &std::collections::BTreeMap::new()) {
            Ok(answer) => format!("<pre>{}</pre>", escape(&answer)),
            Err(error) => format!(
                "<p class=\"note\">The world did not provide its registered operations: {}</p>",
                escape(&error)
            ),
        };
    page(
        "console",
        &format!(
            "<p>Run a registered operation through the same local API as <code>kryta-cli</code>. \
             The world validates the operation and arguments, queues it, and returns the result or \
             refusal. This page does not parse or execute a second command language.</p>\n\
             <form method=\"post\" action=\"/console/run\"><p><label>Operation<br><input required type=\"text\" name=\"operation\" placeholder=\"world.status\"></label></p>\n\
             <p><label>Arguments, one <code>name=value</code> per line<br><textarea name=\"arguments\" placeholder=\"destination=ascalon\"></textarea></label></p>\n\
             <p><button>Run operation</button></p></form>{result}<h2>Registered operations</h2>{operations}"
        ),
    )
}

/// The logs destination reserves monitoring output (roadmap step 4).
pub fn logs() -> Response {
    page(
        "logs",
        "<p>Live service status, server logs, and errors are not implemented yet. Step 4 will present \
         the outputs owned by the existing services and supervisor. This page reads no log.</p>",
    )
}

/// The running world's own answer, asked of its control endpoint.
///
/// Three states are drawn differently, because an operator needs to tell them apart: the endpoint is
/// not configured, it could not be reached or refused, or it answered. An empty answer is a valid
/// result and says so, rather than looking like a failure.
pub fn world_status(manage: &Manage) -> Response {
    match manage.world_status() {
        Ok(answer) if answer.trim().is_empty() => page(
            "world status",
            "<p class=\"note\">The world answered the status operation, and its result is empty.</p>",
        ),
        Ok(answer) => page(
            "world status",
            &format!(
                "<p>The running world answered:</p><pre>{}</pre>",
                escape(&answer)
            ),
        ),
        Err(error) => page(
            "world status",
            &format!(
                "<p class=\"note\">The running world did not answer. This is not an empty result: \
                 the world is unavailable or refused the request.</p><p><code>{}</code></p>",
                escape(&error)
            ),
        ),
    }
}

/// The overview is a work entry point, not a database inventory. It shows the live areas the manager
/// can open and leaves unfinished destinations to state their own limits.
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
    let active = accounts.iter().filter(|account| account.active).count();
    let characters = manage.list_characters();
    let character_count = characters.as_ref().map_or(0, Vec::len);
    let content = manage.list_maps();
    let map_count = content.as_ref().map_or(0, |(maps, _)| maps.len());

    let body = format!(
        "<section class=\"panel\"><div class=\"panel-head\"><div><h2>Operator workspace</h2>\
         <p>Manage the server through its existing owners. Accounts is the active player-management \
         workspace; service control and the shared console remain available in their own areas.</p></div>\
         <a class=\"button\" href=\"/accounts\">Open accounts</a></div></section>\n\
         <section class=\"metric-grid\">\n\
         <div class=\"metric\"><strong>{accounts}</strong><span>accounts</span></div>\n\
         <div class=\"metric\"><strong>{active}</strong><span>active login accounts</span></div>\n\
         <div class=\"metric\"><strong>{characters}</strong><span>characters</span></div>\n\
         <div class=\"metric\"><strong>{maps}</strong><span>map definitions</span></div>\n\
         </section>\n\
         <section class=\"panel\"><div class=\"panel-head\"><div><h2>Working areas</h2>\
         <p>Only the links below open a current manager function. Pending areas keep their own explicit \
         scope rather than simulating an editor.</p></div></div>\n\
         <table><tr><th>area</th><th>current responsibility</th><th>open</th></tr>\n\
         <tr><td>Accounts</td><td>Account lifecycle and account-scoped character records.</td><td><a href=\"/accounts\">open accounts</a></td></tr>\n\
         <tr><td>Services</td><td>Supervisor-owned status, stop, and restart.</td><td><a href=\"/services\">open services</a></td></tr>\n\
         <tr><td>Console</td><td>Registered world operations through the existing API.</td><td><a href=\"/console\">open console</a></td></tr>\n\
         <tr><td>Schema</td><td>Database update records and disk comparison.</td><td><a href=\"/schema\">open schema</a></td></tr>\n\
         </table></section>\n\
         <p class=\"note\">The manager serves local operator use by default. A token in \
         <code>manage.json</code> is required when one is configured.</p>",
        accounts = accounts.len(),
        active = active,
        characters = character_count,
        maps = map_count,
    );
    page("overview", &body)
}

/// The two programs' own settings, read from their files: shown and validated, never rewritten.
///
/// The values are the file's, not the programs' effective ones: a program reads its file at start-up
/// and applies the environment over it, and the file is what an operator shares. The passwords in a
/// connection string are masked by the reader itself, so a page is safe to paste into an issue.
pub fn settings(manage: &Manage) -> Response {
    let (world_path, auth_path) = manage.config_paths();

    let table = |path: Option<&str>| -> String {
        let Some(path) = path else {
            return "<p class=\"note\">No settings file is named in <code>manage.json</code>.</p>\
                    <p class=\"note\">Add <code>WorldConfigFile</code> or <code>AuthConfigFile</code> \
                    to <code>manage.json</code> to show one.</p>"
                .to_string();
        };
        let config = match Config::load_json(path, false, Policy::default()) {
            Ok(config) => config,
            Err(error) => {
                return format!(
                    "<p class=\"note\"><code>{}</code> cannot be read: {}</p>",
                    escape(path),
                    escape(&error.to_string())
                );
            }
        };
        let mut rows: Vec<(String, String)> = config.describe();
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        let mut table = String::new();
        table.push_str("<table>\n<tr><th>key</th><th>value</th></tr>\n");
        for (key, value) in &rows {
            table.push_str(&format!(
                "<tr><td><code>{}</code></td><td><code>{}</code></td></tr>\n",
                escape(key),
                escape(value)
            ));
        }
        table.push_str("</table>\n");
        format!(
            "<p class=\"note\"><code>{}</code> - {} key(s).</p>\n{}",
            escape(path),
            rows.len(),
            table
        )
    };

    let body = format!(
        "<p class=\"note\">Each program's own JSON, read here and never written: an operator shares \
         the file, and the program reads it at start-up. Values are the file's; an environment \
         variable overrides a key at the program's start, not here.</p>\n\
         <h2>the world program</h2>\n{}\n\
         <h2>the login program</h2>\n{}\n",
        table(world_path),
        table(auth_path)
    );
    page("settings", &body)
}

/// Characters remain account-owned. The unfiltered view exists for administration, while an account
/// workspace passes its portal id and receives only that account's records.
pub fn characters(manage: &Manage, account: Option<&str>) -> Response {
    let characters = match account {
        Some(account) => manage.characters_of_account(account),
        None => manage.list_characters(),
    };
    let Some(characters) = characters else {
        return Response::html(
            503,
            message(
                "No character database is configured in manage.json, so there is nothing to list.",
            ),
        );
    };

    let mut rows = String::new();
    for character in &characters {
        let uuid = game::characters::uuid_text(&character.uuid);
        let items = manage
            .list_items_of(&uuid)
            .map_or_else(|| "<em>?</em>".to_string(), |items| items.len().to_string());
        let unlocks = manage.list_unlocks(&uuid).map_or_else(
            || "<em>?</em>".to_string(),
            |unlocked| format!("{} - {unlocked:?}", unlocked.len()),
        );
        rows.push_str(&format!(
            "<tr><td><code>{name}</code></td><td><code>{uuid}</code></td>\
             <td>{campaign}</td><td>{level}</td><td>{outpost}</td><td>{items}</td>\
             <td>{unlocks}</td><td>{pvp}</td>\
             <td><form method=\"post\" action=\"/characters/level\" style=\"display:inline\">\
             <input type=\"hidden\" name=\"uuid\" value=\"{uuid}\">\
             level <input type=\"text\" name=\"level\" size=\"2\" value=\"{level}\">\
             <button type=\"submit\">set</button></form>\
             <form method=\"post\" action=\"/characters/delete\" style=\"display:inline\">\
             <input type=\"hidden\" name=\"uuid\" value=\"{uuid}\">\
             <button type=\"submit\">remove</button></form></td></tr>\n",
            name = escape(&character.name),
            uuid = escape(&uuid),
            campaign = character.settings.campaign,
            level = character.settings.level,
            outpost = character.settings.last_outpost,
            items = items,
            unlocks = unlocks,
            pvp = if character.settings.is_pvp {
                "yes"
            } else {
                "no"
            },
        ));
    }

    let scope = match account {
        Some(account) => format!(
            "<p class=\"note\">Character records for account <code>{}</code>. <a href=\"/accounts\">Return to accounts</a>.</p>",
            escape(account)
        ),
        None => "<p class=\"note\">All character records. Account workspaces open this page filtered to their own characters.</p>".to_string(),
    };
    let body = format!(
        "{scope}<p class=\"note\">Every row of the character table, as the login's own list reads them: a \
         name, the id the client knows it by, its campaign, level, last outpost, how many items it \
         holds, the maps it may travel to (R12), and its PvP flag. A character is written by the \
         creation confirm, not here; its level can be corrected (1 to 20, R48), and removing one \
         deletes its row and its own records (its items and its unlocks).</p>\n\
         <table>\n<tr><th>name</th><th>uuid</th><th>campaign</th><th>level</th><th>last outpost</th>\
         <th>items</th><th>unlocks</th><th>PvP</th><th></th></tr>\n{rows}</table>\n\
         <p class=\"note\">{count} row(s).</p>",
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

/// The content: the world's maps and their spawn and arrival points, and the item templates, read
/// through the nouns' own repositories.
pub fn content(manage: &Manage) -> Response {
    let Some((maps, spawns)) = manage.list_maps() else {
        return Response::html(
            503,
            message(
                "No world database is configured in manage.json, so there is no content to show.",
            ),
        );
    };
    let arrivals = manage.list_arrivals().unwrap_or_default();

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
             <td>{name}</td><td>{party_cap}</td><td>{points}<br><form method=\"post\" action=\"/content/spawn\">\
             <input type=\"hidden\" name=\"map_id\" value=\"{id}\">\
             x <input type=\"text\" name=\"x\" size=\"8\"> \
             y <input type=\"text\" name=\"y\" size=\"8\"> \
             plane <input type=\"text\" name=\"plane\" size=\"3\" value=\"0\"> \
             <button type=\"submit\">add a spawn point</button></form>\
             <form method=\"post\" action=\"/content/party-cap\">\
             <input type=\"hidden\" name=\"map_id\" value=\"{id}\">\
             cap <input type=\"text\" name=\"cap\" size=\"3\" value=\"0\"> \
             <button type=\"submit\">set the party cap (0 = the world's)</button></form></td><td>{arrivals}</td></tr>\n",
            id = map.id,
            kind = map.kind_name(),
            file_id = map.file_id,
            name = escape(&map.name),
            party_cap = match manage.party_cap(map.id) {
                Some(cap) => cap.to_string(),
                None => "<em>the world's</em>".to_string(),
            },
            points = if points.is_empty() {
                "<em>none</em>".to_string()
            } else {
                points.join("<br>")
            },
            arrivals = {
                let points: Vec<String> = arrivals
                    .iter()
                    .filter(|arrival| arrival.map_id == map.id)
                    .map(|arrival| {
                        format!(
                            "from {}: x {:.2} y {:.2} plane {}",
                            arrival.from_map, arrival.x, arrival.y, arrival.plane
                        )
                    })
                    .collect();
                if points.is_empty() {
                    "<em>none</em>".to_string()
                } else {
                    points.join("<br>")
                }
            },
        ));
    }

    let items = manage.list_items().unwrap_or_default();
    let instances = manage.list_instances().unwrap_or_default();
    let sets = manage.creation_sets().unwrap_or_default();
    let mut set_rows = String::new();
    for set in &sets {
        let files: Vec<String> = set
            .items
            .iter()
            .take(6)
            .map(|item| item.file_id.to_string())
            .collect();
        set_rows.push_str(&format!(
            "<tr><td>{campaign}</td><td>{profession}</td><td>{count}</td><td>{files}</td></tr>\n",
            campaign = set.campaign,
            profession = set.profession,
            count = set.items.len(),
            files = files.join(", "),
        ));
    }
    let mut instance_rows = String::new();
    for instance in &instances {
        instance_rows.push_str(&format!(
            "<tr><td>{map_id}</td><td>{district}</td><td>{language}</td><td>{kind}</td>\
             <td>{opened}</td><td>{empty}</td></tr>\n",
            map_id = instance.map_id,
            district = instance.district,
            language = instance.language,
            kind = if instance.kind == 0 {
                "outpost"
            } else {
                "explorable"
            },
            opened = instance.opened_at.clone().unwrap_or_default(),
            empty = instance
                .empty_since
                .clone()
                .unwrap_or_else(|| "somebody inside".to_string()),
        ));
    }
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
         load will not pick again. These rows are read at the world program's start-up: restart it \
         to see an edit take effect.</p>\n\
         <table>\n<tr><th>id</th><th>kind</th><th>file id</th><th>name</th><th>party cap</th>\
         <th>spawn points</th><th>arrivals</th></tr>\n\
         {rows}</table>\n\
         <p class=\"note\">{maps} map(s), {spawns} spawn point(s). Kinds: 0 outpost, 1 explorable, \
         11 character creation.</p>\n\
         <h2>item templates</h2>\n\
         <p class=\"note\">What an item is, once: the rows the creation record lists seed at start-up. \
         A character's own items point at these.</p>\n\
         <table>\n<tr><th>file id</th><th>type</th><th>dyes</th><th>model</th><th>value</th>\
         <th>profession</th><th>stack</th><th>name units</th></tr>\n{item_rows}</table>\n\
         <p class=\"note\">{items} template(s).</p>\n\
         <h2>profession sets</h2>\n\
         <p class=\"note\">The sets a creation screen may choose from, from the creation record \
         list (<code>data/equipment.json</code>): a campaign and a profession, how many items the \
         set carries, and the first few of their <code>Gw.dat</code> file ids. A character is given \
         one of these at its confirm.</p>\n\
         <table>\n<tr><th>campaign</th><th>profession</th><th>items</th><th>first file ids</th></tr>\n\
         {set_rows}</table>\n\
         <p class=\"note\">{sets} set(s).</p>\n\
         <h2>running instances</h2>\n\
         <p class=\"note\">The copies the world has open - the rows its tick disposes once they have \
         been empty past the hold (R37/R40). A copy with a time in the last column is waiting for \
         that.</p>\n\
         <table>\n<tr><th>map</th><th>district</th><th>language</th><th>kind</th><th>opened</th>\
         <th>empty since</th></tr>\n{instance_rows}</table>\n\
         <p class=\"note\">{instances} copy(ies).</p>",
        maps = maps.len(),
        spawns = spawns.len(),
        items = items.len(),
        sets = sets.len(),
        instances = instances.len()
    );
    page("content", &body)
}

/// The account workspace: select an account from the directory, then edit that one record. Keeping
/// the directory separate from the inspector makes a large account set readable and establishes the
/// pattern later player-state editors use.
pub fn accounts(manage: &Manage, selected_id: Option<i64>) -> Response {
    let accounts = match manage.list_accounts() {
        Ok(accounts) => accounts,
        Err(error) => {
            return Response::html(
                500,
                message(&format!("The account table could not be read: {error}")),
            );
        }
    };

    let selected = selected_id.and_then(|id| accounts.iter().find(|account| account.id == id));
    let directory = accounts
        .iter()
        .map(|account| account_directory_row(account, selected_id))
        .collect::<String>();
    let active = accounts.iter().filter(|account| account.active).count();
    let inspector = match selected {
        Some(account) => account_inspector(manage, account),
        None => new_account_inspector(),
    };

    let body = format!(
        "<section class=\"metric-grid\">\n\
         <div class=\"metric\"><strong>{total}</strong><span>accounts</span></div>\n\
         <div class=\"metric\"><strong>{active}</strong><span>active for login</span></div>\n\
         <div class=\"metric\"><strong>{inactive}</strong><span>inactive, retained</span></div>\n\
         </section>\n\
         <section class=\"workspace\">\n\
         <aside class=\"panel directory\"><div class=\"directory-head\"><h2>Account directory</h2>\
         <p>Select an account to manage its login identity and characters.</p></div>\
         <div class=\"directory-list\">{directory}</div></aside>\n\
         {inspector}\n\
         </section>\n\
         <p class=\"note\">Deactivation prevents a later login but retains the account and its \
         character data. It does not terminate a session already in progress. Deletion removes only \
         the account row; character removal is a separate, explicit action.</p>",
        total = accounts.len(),
        active = active,
        inactive = accounts.len().saturating_sub(active),
    );
    page("accounts", &body)
}

/// One compact directory row; edits belong in the selected-account inspector, not in every row.
fn account_directory_row(account: &game::accounts::Account, selected_id: Option<i64>) -> String {
    let selected = if selected_id == Some(account.id) {
        " selected"
    } else {
        ""
    };
    let state = if account.active { "active" } else { "inactive" };
    format!(
        "<a class=\"directory-row{selected}\" href=\"/accounts?account={id}\"><span><strong>{name}</strong>\
         <small>#{id} - {user_id}</small></span><span class=\"badge {state}\">{state}</span></a>",
        id = account.id,
        name = escape(&account.username),
        user_id = escape(&account.user_id),
        selected = selected,
        state = state,
    )
}

/// The selected account's focused editor and account-scoped character entry point.
fn account_inspector(manage: &Manage, account: &game::accounts::Account) -> String {
    let characters = manage.characters_of_account(&account.user_id).map_or_else(
        || "character database unavailable".to_string(),
        |rows| format!("{} character(s)", rows.len()),
    );
    let eula = if account.eula == 0 {
        "not accepted".to_string()
    } else {
        format!("accepted version {}", account.eula)
    };
    let active_action = if account.active {
        "Deactivate account"
    } else {
        "Activate account"
    };
    format!(
        "<section class=\"panel\"><div class=\"panel-head\"><div><h2>{name}</h2>\
         <p>Account #{id} - <span class=\"badge {state}\">{state}</span></p></div>\
         <a class=\"button secondary\" href=\"/characters?account={user_id}\">{characters}</a></div>\
         <form class=\"editor-form\" method=\"post\" action=\"/accounts/update\">\
         <input type=\"hidden\" name=\"id\" value=\"{id}\">\
         <div class=\"field-grid\"><label>Login name<input type=\"text\" name=\"username\" value=\"{name}\"></label>\
         <label>Portal user id<input type=\"text\" name=\"user_id\" value=\"{user_id}\"></label></div>\
         <label>Portal token<input type=\"text\" name=\"token\" value=\"{token}\"></label>\
         <div class=\"actions\"><button type=\"submit\">Save account</button></div></form>\
         <hr class=\"section-rule\"><div class=\"panel-head\"><div><h3>Login state</h3>\
         <p>{eula}. Deactivation affects the next authentication only.</p></div></div>\
         <div class=\"actions\"><form method=\"post\" action=\"/accounts/eula\">\
         <input type=\"hidden\" name=\"username\" value=\"{name}\">\
         <input type=\"hidden\" name=\"version\" value=\"{next_eula}\">\
         <button class=\"secondary\" type=\"submit\">Reset EULA</button></form>\
         <form method=\"post\" action=\"/accounts/active\">\
         <input type=\"hidden\" name=\"id\" value=\"{id}\">\
         <input type=\"hidden\" name=\"active\" value=\"{next_active}\">\
         <button class=\"secondary\" type=\"submit\">{active_action}</button></form></div>\
         <hr class=\"section-rule\"><div class=\"panel-head\"><div><h3>Permanent removal</h3>\
         <p>Removes this login account only. Its character records are retained and can be removed from \
         the character workspace when explicitly intended.</p></div></div>\
         <form method=\"post\" action=\"/accounts/delete\" onsubmit=\"return confirm('Remove {name}? Its characters are retained.')\">\
         <input type=\"hidden\" name=\"username\" value=\"{name}\"><button class=\"danger\" type=\"submit\">Remove account</button></form></section>",
        id = account.id,
        name = escape(&account.username),
        user_id = escape(&account.user_id),
        token = escape(&account.token),
        state = if account.active { "active" } else { "inactive" },
        characters = characters,
        eula = eula,
        next_eula = if account.eula == 0 { 26 } else { 0 },
        next_active = !account.active,
        active_action = active_action,
    )
}

/// The inspector's empty state is also the account creation form.
fn new_account_inspector() -> String {
    "<section class=\"panel\"><div class=\"panel-head\"><div><h2>Create account</h2>\
     <p>Create a login account, then select it to reach its characters.</p></div></div>\
     <form class=\"editor-form\" method=\"post\" action=\"/accounts/create\">\
     <label>Login name<input type=\"text\" name=\"username\" placeholder=\"name@example.com\"></label>\
     <div class=\"field-grid\"><label>Portal user id<input type=\"text\" name=\"user_id\" placeholder=\"generated when blank\"></label>\
     <label>Portal token<input type=\"text\" name=\"token\" placeholder=\"generated when blank\"></label></div>\
     <div class=\"actions\"><button type=\"submit\">Create account</button></div></form>\
     <p class=\"note\">The portal issues the identity pair the client presents. Leave either value \
     blank only when this local manager should generate it.</p></section>"
        .to_string()
}

/// The schema: each database's own record of the files it has applied, against the files on disk.
pub fn schema(manage: &Manage) -> Response {
    let mut sections = String::new();
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
    Response::new(
        303,
        "text/html; charset=utf-8",
        format!("<p>moved to <a href=\"{to}\">{to}</a></p>").into_bytes(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_unconnected_manager_destination_has_the_shared_navigation_and_states_its_scope() {
        for (name, destination) in [
            ("configuration", configuration as fn() -> Response),
            ("assets", assets),
            ("logs", logs),
        ] {
            let response = destination();
            let body = String::from_utf8(response.body).expect("the page is UTF-8");
            assert_eq!(response.status, 200, "{name}");
            for path in [
                "/services",
                "/accounts",
                "/configuration",
                "/assets",
                "/console",
                "/logs",
            ] {
                assert!(body.contains(path), "{name} omits {path}");
            }
            assert!(body.contains("not implemented yet"), "{name}");
        }
    }

    #[test]
    fn a_service_action_is_built_only_from_the_supervisors_reported_names() {
        let body = service_actions("auth: up on 6112\nworld: up on 6113\nnot a service line");
        assert!(body.contains("/services/restart"));
        assert!(body.contains("/services/stop"));
        assert!(body.contains("name=\"service\" value=\"world\""));
        assert!(!body.contains("not a service line"));
    }

    #[test]
    fn navigation_groups_the_operator_work_and_marks_the_current_area() {
        let accounts = navigation("accounts");
        assert!(accounts.contains("workspace"));
        assert!(accounts.contains("players"));
        assert!(accounts.contains("server data"));
        assert!(accounts.contains("nav-link active\" href=\"/accounts\""));
        assert!(!accounts.contains("nav-link active\" href=\"/services\""));
    }

    #[test]
    fn an_account_directory_row_selects_one_account_without_exposing_every_edit() {
        let row = account_directory_row(
            &game::accounts::Account {
                id: 42,
                username: "operator@example.com".to_string(),
                user_id: "user".to_string(),
                token: "token".to_string(),
                eula: 26,
                active: true,
            },
            Some(42),
        );
        assert!(row.contains("/accounts?account=42"));
        assert!(row.contains("directory-row selected"));
        assert!(row.contains("operator@example.com"));
        assert!(!row.contains("/accounts/update"));
        assert!(!row.contains("/accounts/delete"));
    }
}
