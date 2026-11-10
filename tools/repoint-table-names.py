"""Point each split table's name module at the shared vocabulary instead of its old parent.

`split-protocol-table.py` cut the merged table into a world half and a login half, and each half
carried the merged file's name module (`pub mod game`, `pub mod auth`) with it. That module reached
its siblings with `super::`, which is no longer where they live: the types are
`shared::packets::types`, and the rows are the module above it. This script rewrites those paths, and
is idempotent.

Usage: python tools/repoint-table-names.py
"""

import io
import sys

FILES = (
    ("game/src/Server/Protocol/Tables.rs", "game", "crate::server::protocol::tables"),
    ("authserver/src/Protocol/Tables.rs", "auth", "crate::protocol::tables"),
)


def main() -> int:
    for path, family, own_path in FILES:
        with io.open(path, encoding="utf-8", newline="") as handle:
            text = handle.read()

        before = text
        text = text.replace(
            "    use super::{Conn, lookup as packet_lookup};\n", ""
        ).replace(
            "    use super::{lookup as packet_lookup, Conn};\n", ""
        )
        text = text.replace(
            "    pub use super::Packet as Entry;",
            "    pub use shared::packets::types::Packet as Entry;",
        )
        text = text.replace(
            "    pub use super::{Field, FieldKind, declared_max_size};",
            "    pub use shared::packets::types::{Field, FieldKind, declared_max_size};",
        )
        text = text.replace(
            "    pub use super::{Field, FieldKind};",
            "    pub use shared::packets::types::{Field, FieldKind};",
        )
        text = text.replace("        super::rows()", "        %s::rows()" % own_path)
        text = text.replace(
            "        packet_lookup(Conn::Game, header, from_client)",
            "        %s::lookup(header, from_client)" % own_path,
        )
        text = text.replace(
            "        packet_lookup(Conn::Auth, header, from_client)",
            "        %s::lookup(header, from_client)" % own_path,
        )
        text = text.replace(
            "        lookup(header, from_client).map(|entry| entry.fields)",
            "        %s::lookup(header, from_client).map(|entry| entry.fields)" % own_path,
        )
        text = text.replace(
            "        lookup(header, from_client).map(|entry| entry.size)",
            "        %s::lookup(header, from_client).map(|entry| entry.size)" % own_path,
        )

        if text != before:
            with io.open(path, "w", encoding="utf-8", newline="") as handle:
                handle.write(text)
            print("repointed", path, "(%s)" % family)
        else:
            print("already repointed", path)

    return 0


if __name__ == "__main__":
    sys.exit(main())
