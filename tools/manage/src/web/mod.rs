//! The web layer: the HTTP helpers every surface shares, the shell every page is drawn into, and
//! the handful of pieces a page is built from.
//!
//! The panel is deliberately plain: a page is a function from the tool's state to an HTML body, and
//! nothing is fetched beyond the page itself - the stylesheet is served from this program's memory,
//! and there is no JavaScript except the two probes that ask the running world and supervisor for
//! state after the page has already answered (their owners are separate processes, and a page must
//! never wait for one to be up).

pub mod components;
pub mod http;
pub mod layout;
