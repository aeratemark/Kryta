//! `game/Pack/` - the `Gw.dat` reader: the client's asset archive, and what is inside it.
//!
//! Reference: no directory of its own. The reference reads this file with `FaArchive.c` and
//! `FaCompress.c`, two files of its `src/common/` - but this tree's `common/` is the library both
//! programs link and a 3.6 GB archive is nobody's common dependency, so the reader lives in the game
//! library as the `Pack` noun.
//!
//! What it is for: a map's geometry - the path graph a walk is answered from - is a chunk inside one of
//! the archive's entries, and a map's file id (the number `0x0196` carries) is the way in. `Gwdat.rs` is
//! the index that turns a file id into an entry; the decoder that turns an entry into bytes is the next
//! piece of this noun.

#[path = "Gwdat.rs"]
pub mod gwdat;

pub use gwdat::{Entry, GwDat, Header, MAGIC};
