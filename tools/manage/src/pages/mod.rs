//! The panel's pages: one module per destination, each a function from the tool's state to an
//! HTML body.
//!
//! The destinations follow the game's ownership, not the database layout. `accounts` is the accounts
//! the server hosts and the state bound to the person; `players` is the characters those accounts own
//! (aliased from `accounts` and listed across them), whose own pages carry their progress, position,
//! inventory, titles and unlocks. `services`, `console` and `status` are server administration, and
//! the world's content, realms, settings and schema are read through the nouns' own repositories.

pub mod accounts;
pub mod console;
pub mod content;
pub mod navigation;
pub mod overview;
pub mod players;
pub mod schema;
pub mod services;
pub mod settings;
pub mod status;
pub mod worlds;

use shared::network::http::Response;

use crate::web::{components, layout};

/// A page that says one thing, in the panel's own frame: an error, or the reason a request was
/// refused. The status code is the answer's, and the title is what the rail would have shown.
pub fn error(status: u16, title: &str, text: &str) -> Response {
    layout::page_with_status(
        status,
        title,
        "",
        &components::note(&components::escape(text)),
    )
}

/// The 401 page.
pub fn unauthorised() -> Response {
    error(
        401,
        "unauthorised",
        "This tool wants its token: add ?token=<value> to the address once.",
    )
}

/// The 405 page: an address that exists answers only the methods its page takes.
pub fn method_not_allowed(path: &str) -> Response {
    error(
        405,
        "method not allowed",
        &format!("No page at {path} answers this method."),
    )
}

/// The 404 page.
pub fn missing(path: &str) -> Response {
    error(404, "not found", &format!("No page is at {path}."))
}
