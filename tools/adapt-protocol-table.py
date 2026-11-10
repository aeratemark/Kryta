"""Adapt the copied packet table to its new home inside this crate.

The table was written as a crate root (`crate::packets`), so its own module bodies reach their
siblings with `super::`. Here it is one module beside the rest of the protocol, so those become
`self::`, and the doc links that named the old crate path name this one.

Run once after copying; it is idempotent.
"""

import io
import sys

PATH = "game/src/Server/Protocol/Tables.rs"

REPLACEMENTS = (
    ("use super::{lookup, Conn};", "use self::{Conn, lookup};"),
    ("use super::{lookup as packet_lookup, Conn};", "use self::{Conn, lookup as packet_lookup};"),
    ("pub use super::Packet as Entry;", "pub use self::Packet as Entry;"),
    ("pub use super::{Field, FieldKind};", "pub use self::{Field, FieldKind};"),
    ("pub use super::{declared_max_size, Field, FieldKind};", "pub use self::{Field, FieldKind, declared_max_size};"),
    ("super::rows(Conn::Auth)", "self::rows(Conn::Auth)"),
    ("super::rows(Conn::Game)", "self::rows(Conn::Game)"),
    ("[`Status`](crate::packets::Status)", "[`Status`](Status)"),
    ("[`progress`](crate::packets::progress)", "[`progress`](progress)"),
    ("[`crate::game`]", "[`crate::game`](crate::game)"),
)


def main() -> int:
    with io.open(PATH, encoding="utf-8", newline="") as handle:
        text = handle.read()

    changed = 0
    for old, new in REPLACEMENTS:
        if old in text:
            text = text.replace(old, new)
            changed += 1

    with io.open(PATH, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)

    print("replacements applied:", changed)
    return 0


if __name__ == "__main__":
    sys.exit(main())
