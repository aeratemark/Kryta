"""Move the protocol table's *types* out of the table and into the shared wire module.

What both connections need is the mechanism: the field vocabulary (`Field`, `FieldKind`, `Packet`,
`Conn`, `Status`) and the arithmetic a declared size comes from. What each program owns is its rows.
This script is the move that follows `split-protocol-table.py`: it writes
`shared/src/Packets/Types.rs` from the merged table and leaves `shared/src/Packets/Tables.rs` gone.

Input:  shared/src/Packets/Tables.rs (the merged file, before the split removed the rows)
        The rows have already been written to each program by `split-protocol-table.py`, so this
        reads the *type* sections of the same file and writes them to Types.rs.

Run once; it is the record of the move.
"""

import io
import re
import sys

SOURCE = "shared/src/Packets/Tables.rs"
OUT = "shared/src/Packets/Types.rs"


def main():
    with io.open(SOURCE, encoding="utf-8", newline="") as handle:
        text = handle.read()

    # 1. The type block: from `Conn` to the end of `Packet`'s impl.
    start = text.index("/// Which connection a message belongs to.")
    end = text.index("/// The one table.")
    types = text[start:end].rstrip() + "\n"

    # 2. The arithmetic a declared size comes from.
    helper_start = text.index("pub fn declared_max_size(fields: &[Field]) -> usize {")
    doc_start = text.rindex("///", 0, helper_start)
    helper_end = text.index("/// The auth family's accessor surface")
    helper = text[doc_start:helper_end].rstrip() + "\n"

    header = '''//! The protocol's vocabulary: a message, its fields, and which connection it belongs to.
//!
//! Shared, because both connections are framed by the same walk (`Frame` beside this file) and the
//! auth program does not link the game library. The *rows* are not here: a program owns the rows of
//! the connection it serves - `game/src/Server/Protocol/Tables.rs` for a world connection and
//! `authserver/src/Protocol/Tables.rs` for the login - which is the reference's arrangement.
//!
//! Ported from `ASURA/crates/asura-protocol/src/packets.rs`; `tools/split-protocol-table.py` records
//! how the merged file was separated again.

'''

    with io.open(OUT, "w", encoding="utf-8", newline="") as handle:
        handle.write(header + types + "\n" + helper)
    print("wrote", OUT)
    return 0


if __name__ == "__main__":
    sys.exit(main())
