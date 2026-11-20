//! The shell every page is written into.
//!
//! It is an operator workspace: information stays compact, the active work area is explicit, and
//! every page inherits the same frame. The stylesheet is compiled into the program and served by
//! the routes, so a page is one request and nothing waits on an asset host or a network.

use shared::network::http::Response;

/// The panel's stylesheet, served at `/static/style.css`.
pub const STYLE: &str = include_str!("style.css");

/// The content type the stylesheet is served with.
pub const STYLE_TYPE: &str = "text/css; charset=utf-8";

/// Where the rail's groups and their destinations are.
///
/// The order is the game's own: what the operator is looking at first (the workspace), then the
/// players whose accounts own characters, then the world they are in, then the server that serves
/// it. A destination that does not work is not in this list: the rail is not a roadmap.
const GROUPS: [(&str, &[(&str, &str, &str)]); 4] = [
    ("workspace", &[("/", "Overview", "overview")]),
    ("players", &[("/players", "Players", "players")]),
    (
        "world",
        &[
            ("/content", "World content", "content"),
            ("/worlds", "Worlds", "worlds"),
            ("/navigation", "Navigation", "navigation"),
        ],
    ),
    (
        "server",
        &[
            ("/services", "Services", "services"),
            ("/console", "Console", "console"),
            ("/status", "World status", "status"),
            ("/settings", "Settings", "settings"),
            ("/schema", "Schema", "schema"),
        ],
    ),
];

/// The shell: the document head, the rail with the current area marked, and the page's body.
pub fn page(title: &str, active: &str, body: &str) -> Response {
    render(200, title, active, body)
}

/// The same shell with another status, for a page that says why it is not the one asked for.
pub fn page_with_status(status: u16, title: &str, active: &str, body: &str) -> Response {
    render(status, title, active, body)
}

fn render(status: u16, title: &str, active: &str, body: &str) -> Response {
    let html = format!(
        "<!doctype html>\n\
         <html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Kryta Control - {title}</title>\n\
         <link rel=\"stylesheet\" href=\"/static/style.css\">\n\
         </head><body><div class=\"shell\">\n\
         <aside class=\"rail\"><a class=\"brand\" href=\"/\"><span class=\"brand-mark\">K</span>\
         <span><strong>Kryta Control</strong><small>Configuration Manager</small></span></a>\n\
         <nav aria-label=\"Kryta Control\">{navigation}</nav>\n\
         <p class=\"rail-foot\">Local operator workspace<br>Server ownership stays with its services.</p></aside>\n\
         <main><header class=\"page-head\"><div><p class=\"eyebrow\">Kryta Control</p><h1>{title}</h1></div>\
         <span class=\"local-mark\">local operator surface</span></header>\n{body}\n</main></div></body></html>\n",
        navigation = navigation(active)
    );
    Response::html(status, html)
}

/// The rail: every stable destination, grouped by whose data it is, the current area marked.
fn navigation(active: &str) -> String {
    GROUPS
        .iter()
        .map(|(label, items)| {
            let links = items
                .iter()
                .map(|(path, name, key)| {
                    let class = if *key == active { " active" } else { "" };
                    format!("<a class=\"nav-link{class}\" href=\"{path}\">{name}</a>")
                })
                .collect::<String>();
            format!("<div class=\"nav-group\"><p class=\"rail-label\">{label}</p>{links}</div>")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rail_groups_what_the_operator_works_with_and_marks_the_current_area() {
        let players = navigation("players");
        assert!(players.contains("workspace"));
        assert!(players.contains("players"));
        assert!(players.contains("world"));
        assert!(players.contains("server"));
        assert!(players.contains("href=\"/content\""));
        assert!(players.contains("href=\"/players\""));
        assert!(players.contains("nav-link active\" href=\"/players\""));
        assert!(!players.contains("nav-link active\" href=\"/services\""));

        let services = navigation("services");
        assert!(services.contains("nav-link active\" href=\"/services\""));
    }

    #[test]
    fn the_shell_carries_the_stylesheet_and_the_areas_title() {
        let response = page("Overview", "overview", "<p>body</p>");
        let body = String::from_utf8(response.body).expect("the page is UTF-8");
        assert_eq!(response.status, 200);
        assert!(body.contains("href=\"/static/style.css\""));
        assert!(body.contains("<h1>Overview</h1>"));
        assert!(body.contains("<p>body</p>"));
    }

    #[test]
    fn the_stylesheet_is_the_style_the_panel_draws() {
        assert!(STYLE.contains("--accent:#e7a55e;") || STYLE.contains("--accent: #e7a55e;"));
        assert!(STYLE.contains(".rail"));
    }
}
