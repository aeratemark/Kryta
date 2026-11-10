//! The key exchange: this module's name, kept, over the copy both conversations share.
//!
//! The arithmetic moved to `shared/src/Cryptography/Handshake.rs` when the gameplay connection
//! needed it too: the world makes exactly the same exchange as the login, because it is the same
//! client protocol on a second socket. Nothing here changed but the home - the constants, the
//! arithmetic and their tests are the ones this file used to hold (`AGENTS.md`, rule 11).
//!
//! Reference: the exchange's shape is `AuthSocket::HandleAuthChallenge`, and the cipher a connection
//! runs afterwards is `src/server/shared/Cryptography/AuthCrypt`.

pub use shared::cryptography::handshake::*;
