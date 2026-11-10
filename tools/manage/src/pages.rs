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
         <nav><a href=\"/\">overview</a><a href=\"/accounts\">accounts</a></nav>\n\
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

    let body = format!(
        "<p>The administration tool. It writes the same rows the servers read, through the same \
         code: nothing here is a second implementation of the data.</p>\n\
         <table>\n\
         <tr><th>what</th><th>where</th><th>state</th></tr>\n\
         <tr><td>accounts</td><td><code>kryta_login.account</code></td><td>{} row(s), {accepted} with an EULA accepted</td></tr>\n\
         <tr><td>characters</td><td><code>kryta_characters.characters</code></td><td>a page is next</td></tr>\n\
         <tr><td>worlds</td><td><code>realmlist</code> + <code>data/realms.json</code></td><td>a page is next</td></tr>\n\
         <tr><td>content</td><td><code>data/maps.json</code>, <code>data/inventory.json</code>, <code>data/equipment.json</code></td><td>a page is next</td></tr>\n\
         </table>\n\
         <p class=\"note\">The tool serves this machine only, by default. A token in \
         <code>manage.json</code> makes it require one.</p>",
        accounts.len()
    );
    page("overview", &body)
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
